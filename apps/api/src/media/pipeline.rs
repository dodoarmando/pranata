//! Image pipeline (D-16, ADR 0006): sniff, bound, decode, orient, strip and
//! derive WebP variants. Pure CPU work on byte slices; callers run it on a
//! blocking thread.
//!
//! No metadata survives: the stored original and every variant are encoded
//! from decoded pixels, so EXIF (including GPS) and ICC profiles are dropped.

use std::io::Cursor;

use fast_image_resize::{
    FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer, images::Image as FirImage,
};
use image::{
    DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, codecs::jpeg::JpegEncoder,
    metadata::Orientation,
};

use crate::ApiError;

/// Public variant widths, never exceeded by upscaling.
pub const WIDTHS: [u32; 4] = [480, 960, 1600, 2560];

/// Largest accepted image, in pixels (width times height).
pub const MAX_PIXELS: u64 = 50_000_000;

/// Quality of the public lossy WebP variants.
pub const WEBP_QUALITY: f32 = 78.0;

/// Quality of the metadata-free re-encoded original (JPEG and WebP).
const ORIGINAL_QUALITY: u8 = 90;

/// Second line of defence behind the header check: decoders may allocate at
/// most this much, and no side may exceed [`MAX_SIDE`].
const MAX_DECODE_ALLOC: u64 = 512 * 1024 * 1024;
const MAX_SIDE: u32 = 50_000;

/// A file type recognised from its first bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Jpeg,
    Png,
    Webp,
    Pdf,
}

impl Format {
    /// Recognise a file by its magic bytes; the name and declared type of an
    /// upload are never trusted.
    #[must_use]
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            Some(Self::Webp)
        } else if bytes.starts_with(b"%PDF-") {
            Some(Self::Pdf)
        } else {
            None
        }
    }

    #[must_use]
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
            Self::Pdf => "application/pdf",
        }
    }

    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Pdf => "pdf",
        }
    }

    #[must_use]
    pub const fn is_image(self) -> bool {
        !matches!(self, Self::Pdf)
    }

    /// The format for a stored MIME type.
    #[must_use]
    pub fn from_mime(mime: &str) -> Option<Self> {
        [Self::Jpeg, Self::Png, Self::Webp, Self::Pdf]
            .into_iter()
            .find(|format| format.mime() == mime)
    }

    fn image_format(self) -> Option<ImageFormat> {
        match self {
            Self::Jpeg => Some(ImageFormat::Jpeg),
            Self::Png => Some(ImageFormat::Png),
            Self::Webp => Some(ImageFormat::WebP),
            Self::Pdf => None,
        }
    }
}

/// What the header of an accepted image says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inspected {
    pub format: Format,
    /// Width and height as stored in the file, before any orientation.
    pub width: u32,
    pub height: u32,
}

/// Steps 1 and 2: sniff the type and read the dimensions from the header
/// without decoding any pixels.
///
/// # Errors
/// 415 `unsupported_media_type` for anything but JPEG, PNG or WebP; 422
/// `image_unreadable` when the header cannot be read; 422 `image_too_large`
/// above [`MAX_PIXELS`].
pub fn inspect(bytes: &[u8]) -> Result<Inspected, ApiError> {
    let format = Format::sniff(bytes)
        .filter(|format| format.is_image())
        .ok_or(ApiError::Unsupported("unsupported_media_type"))?;
    let image_format = format
        .image_format()
        .ok_or(ApiError::Unsupported("unsupported_media_type"))?;
    let (width, height) = ImageReader::with_format(Cursor::new(bytes), image_format)
        .into_dimensions()
        .map_err(|_| ApiError::Unprocessable("image_unreadable"))?;
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(ApiError::Unprocessable("image_too_large"));
    }
    Ok(Inspected {
        format,
        width,
        height,
    })
}

/// One public WebP rendition.
#[derive(Debug)]
pub struct Variant {
    pub width: u32,
    pub height: u32,
    pub webp: Vec<u8>,
}

/// The result of processing an upload.
#[derive(Debug)]
pub struct Processed {
    /// The original re-encoded without metadata, in its own format.
    pub original: Vec<u8>,
    pub format: Format,
    /// Dimensions after the EXIF orientation is applied.
    pub width: u32,
    pub height: u32,
    pub variants: Vec<Variant>,
}

/// The variant widths for an image `width` pixels wide: every standard width
/// that does not exceed it, plus its own width when that is below the largest
/// standard one, so the biggest public image is as sharp as the source allows.
#[must_use]
pub fn variant_widths(width: u32) -> Vec<u32> {
    let mut widths: Vec<u32> = WIDTHS.into_iter().filter(|w| *w <= width).collect();
    if width < WIDTHS[WIDTHS.len() - 1] && !widths.contains(&width) {
        widths.push(width);
    }
    widths
}

fn internal(error: impl std::fmt::Display) -> ApiError {
    ApiError::Internal(anyhow::anyhow!("{error}"))
}

/// Run the whole pipeline on an uploaded image.
///
/// # Errors
/// The errors of [`inspect`]; 422 `image_undecodable` when the pixel data is
/// broken; an internal error when encoding fails.
pub fn process_image(original: &[u8]) -> Result<Processed, ApiError> {
    let inspected = inspect(original)?;
    let image_format = inspected
        .format
        .image_format()
        .ok_or(ApiError::Unsupported("unsupported_media_type"))?;

    // Decoders fill in missing JPEG data silently; a photo cut off by a dropped
    // connection must fail instead of becoming a half-grey image.
    if inspected.format == Format::Jpeg && !jpeg_is_complete(original) {
        tracing::warn!("rejecting a JPEG without an end-of-image marker (truncated upload)");
        return Err(ApiError::Unprocessable("image_truncated"));
    }

    // Step 3: decode with limits, then apply the EXIF orientation.
    let mut reader = ImageReader::with_format(Cursor::new(original), image_format)
        .into_decoder()
        .map_err(|_| ApiError::Unprocessable("image_undecodable"))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader
        .set_limits(limits)
        .map_err(|_| ApiError::Unprocessable("image_too_large"))?;
    let orientation = reader.orientation().unwrap_or(Orientation::NoTransforms);
    let mut decoded = DynamicImage::from_decoder(reader)
        .map_err(|_| ApiError::Unprocessable("image_undecodable"))?;
    decoded.apply_orientation(orientation);
    let (width, height) = (decoded.width(), decoded.height());
    let alpha = decoded.color().has_alpha();

    // Step 4: re-encode the original from pixels, which drops all metadata.
    let stored = encode_original(&decoded, inspected.format, alpha)?;

    // Step 5: resize each width from the same source buffer and encode WebP.
    let (source, pixel_type) = if alpha {
        (decoded.into_rgba8().into_raw(), PixelType::U8x4)
    } else {
        (decoded.into_rgb8().into_raw(), PixelType::U8x3)
    };
    let source = FirImage::from_vec_u8(width, height, source, pixel_type).map_err(internal)?;
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));
    let mut resizer = Resizer::new();
    let mut variants = Vec::new();
    for target in variant_widths(width) {
        let target_height = scaled_height(width, height, target);
        let pixels = if target == width {
            source.buffer().to_vec()
        } else {
            let mut scaled = FirImage::new(target, target_height, pixel_type);
            resizer
                .resize(&source, &mut scaled, &options)
                .map_err(internal)?;
            scaled.into_vec()
        };
        variants.push(Variant {
            width: target,
            height: target_height,
            webp: encode_webp(&pixels, target, target_height, alpha, WEBP_QUALITY)?,
        });
    }

    Ok(Processed {
        original: stored,
        format: inspected.format,
        width,
        height,
        variants,
    })
}

/// Whether a JPEG stream reaches its end-of-image marker. It walks the marker
/// segments, so an `FF D9` inside an embedded thumbnail does not count, and
/// data after the marker (motion photo videos, padding) is fine.
fn jpeg_is_complete(bytes: &[u8]) -> bool {
    let mut at = 2; // after the start-of-image marker
    while at + 1 < bytes.len() {
        if bytes[at] != 0xFF {
            return false;
        }
        let marker = bytes[at + 1];
        match marker {
            0xFF => at += 1,               // fill byte
            0xD9 => return true,           // end of image
            0x01 | 0xD0..=0xD8 => at += 2, // markers without a length
            _ => {
                let Some(length) = bytes.get(at + 2..at + 4) else {
                    return false;
                };
                at += 2 + usize::from(u16::from_be_bytes([length[0], length[1]]));
                if marker == 0xDA {
                    // Entropy-coded data: skip it up to the next real marker
                    // (`FF 00` is a stuffed byte, `FF D0..D7` a restart).
                    while at + 1 < bytes.len()
                        && !(bytes[at] == 0xFF
                            && bytes[at + 1] != 0x00
                            && !(0xD0..=0xD7).contains(&bytes[at + 1]))
                    {
                        at += 1;
                    }
                }
            }
        }
    }
    false
}

fn scaled_height(width: u32, height: u32, target: u32) -> u32 {
    let scaled = (u64::from(height) * u64::from(target) + u64::from(width) / 2) / u64::from(width);
    u32::try_from(scaled).unwrap_or(u32::MAX).max(1)
}

fn encode_original(image: &DynamicImage, format: Format, alpha: bool) -> Result<Vec<u8>, ApiError> {
    let mut out = Vec::new();
    match format {
        Format::Jpeg => JpegEncoder::new_with_quality(&mut out, ORIGINAL_QUALITY)
            .encode_image(&image.to_rgb8())
            .map_err(internal)?,
        Format::Png => image
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .map_err(internal)?,
        Format::Webp => {
            let (w, h) = (image.width(), image.height());
            out = if alpha {
                encode_webp(
                    image.to_rgba8().as_raw(),
                    w,
                    h,
                    true,
                    f32::from(ORIGINAL_QUALITY),
                )?
            } else {
                encode_webp(
                    image.to_rgb8().as_raw(),
                    w,
                    h,
                    false,
                    f32::from(ORIGINAL_QUALITY),
                )?
            };
        }
        Format::Pdf => return Err(ApiError::Unsupported("unsupported_media_type")),
    }
    Ok(out)
}

fn encode_webp(
    pixels: &[u8],
    width: u32,
    height: u32,
    alpha: bool,
    quality: f32,
) -> Result<Vec<u8>, ApiError> {
    let encoder = if alpha {
        webp::Encoder::from_rgba(pixels, width, height)
    } else {
        webp::Encoder::from_rgb(pixels, width, height)
    };
    let memory = encoder
        .encode_simple(false, quality)
        .map_err(|error| internal(format!("webp encoding failed: {error:?}")))?;
    Ok(memory.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_never_exceed_the_source_and_end_at_its_own_width() {
        assert_eq!(variant_widths(300), [300]);
        assert_eq!(variant_widths(480), [480]);
        assert_eq!(variant_widths(1000), [480, 960, 1000]);
        assert_eq!(variant_widths(1600), [480, 960, 1600]);
        assert_eq!(variant_widths(2000), [480, 960, 1600, 2000]);
        assert_eq!(variant_widths(2560), [480, 960, 1600, 2560]);
        assert_eq!(variant_widths(6000), [480, 960, 1600, 2560]);
    }

    #[test]
    fn sniffing_uses_magic_bytes_only() {
        assert_eq!(Format::sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(Format::Jpeg));
        assert_eq!(Format::sniff(b"\x89PNG\r\n\x1a\nrest"), Some(Format::Png));
        assert_eq!(Format::sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some(Format::Webp));
        assert_eq!(Format::sniff(b"%PDF-1.7"), Some(Format::Pdf));
        assert_eq!(Format::sniff(b"\0\0\0\x18ftypheic"), None);
        assert_eq!(Format::sniff(b"GIF89a"), None);
        assert_eq!(Format::sniff(b""), None);
    }

    #[test]
    fn jpeg_completeness_follows_the_marker_structure() {
        // SOI, APP0 (length 4), SOS (length 3), entropy data with a stuffed
        // byte and a restart marker, EOI.
        let whole = [
            0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 1, 2, 0xFF, 0xDA, 0, 3, 0, 0xAA, 0xFF, 0x00, 0xBB, 0xFF,
            0xD0, 0xCC, 0xFF, 0xD9,
        ];
        assert!(jpeg_is_complete(&whole));
        let mut padded = whole.to_vec();
        padded.extend([0, 0, 0, b'f', b't', b'y', b'p']);
        assert!(jpeg_is_complete(&padded));
        assert!(!jpeg_is_complete(&whole[..whole.len() - 2]));
        assert!(!jpeg_is_complete(&whole[..12]));
        assert!(!jpeg_is_complete(&[0xFF, 0xD8]));
    }

    #[test]
    fn heights_keep_the_aspect_ratio_and_never_reach_zero() {
        assert_eq!(scaled_height(1000, 600, 480), 288);
        assert_eq!(scaled_height(5000, 1, 480), 1);
    }
}
