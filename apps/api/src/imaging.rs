//! The image pipeline: an upload becomes a metadata-free master and its public variants.

use std::io::Cursor;

use image::{
    DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, RgbImage, RgbaImage,
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
    imageops::FilterType,
};
use resvg::{tiny_skia, usvg};

/// What an upload is for, which fixes the types and size it may have.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Kind {
    /// JPEG, PNG or WebP, up to 10 MiB.
    Photo,
    /// A [`Kind::Photo`] that also gets the 2400 px variant, for the home page.
    Hero,
    /// PNG or SVG, up to 2 MiB; an SVG is rasterised and never kept.
    Logo,
}

impl Kind {
    pub(crate) fn max_bytes(self) -> usize {
        match self {
            Self::Photo | Self::Hero => 10 << 20,
            Self::Logo => 2 << 20,
        }
    }

    /// The variants stored for an upload of this kind.
    pub(crate) fn variants(self) -> &'static [Variant] {
        match self {
            Self::Hero => &Variant::HERO,
            Self::Photo | Self::Logo => &Variant::ALL,
        }
    }
}

/// Why an upload cannot become an image.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Rejected {
    /// Not one of the types its [`Kind`] accepts, judged by content, not by name.
    UnsupportedType,
    /// More than [`MAX_PIXELS`] once decoded.
    TooManyPixels,
    /// The type is accepted but the content does not decode.
    Corrupt,
}

/// The decoded-pixel cap, checked from the header before anything is decoded.
pub(crate) const MAX_PIXELS: u64 = 40_000_000;
/// The master's longest edge.
const MASTER_EDGE: u32 = 2560;

/// A public rendition of an image, served at `/media/{id}/{name}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Variant {
    W480,
    W960,
    W1600,
    /// Only for [`Kind::Hero`].
    W2400,
    /// The 1200 px JPEG for `og:image`, opaque on white.
    Share,
}

impl Variant {
    /// Every Variant made for every image.
    pub(crate) const ALL: [Self; 4] = [Self::W480, Self::W960, Self::W1600, Self::Share];
    /// Every Variant made for a [`Kind::Hero`], and so every Variant there is.
    pub(crate) const HERO: [Self; 5] = [
        Self::W480,
        Self::W960,
        Self::W1600,
        Self::W2400,
        Self::Share,
    ];

    /// The name in the URL.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::W480 => "480",
            Self::W960 => "960",
            Self::W1600 => "1600",
            Self::W2400 => "2400",
            Self::Share => "share",
        }
    }

    pub(crate) fn file_name(self) -> &'static str {
        match self {
            Self::W480 => "480.webp",
            Self::W960 => "960.webp",
            Self::W1600 => "1600.webp",
            Self::W2400 => "2400.webp",
            Self::Share => "share.jpg",
        }
    }

    pub(crate) fn content_type(self) -> &'static str {
        match self {
            Self::Share => "image/jpeg",
            _ => "image/webp",
        }
    }

    fn width(self) -> u32 {
        match self {
            Self::W480 => 480,
            Self::W960 => 960,
            Self::W1600 => 1600,
            Self::W2400 => 2400,
            Self::Share => 1200,
        }
    }
}

/// Decodes an upload of `kind` upright, at most [`MASTER_EDGE`] on its longest edge.
pub(crate) fn decode(kind: Kind, bytes: &[u8]) -> Result<DynamicImage, Rejected> {
    let format = image::guess_format(bytes).ok();
    let image = match (kind, format) {
        (
            Kind::Photo | Kind::Hero,
            Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP),
        )
        | (Kind::Logo, Some(ImageFormat::Png)) => decode_raster(bytes)?,
        (Kind::Logo, None) => rasterise_svg(bytes)?,
        _ => return Err(Rejected::UnsupportedType),
    };
    Ok(fit(image, MASTER_EDGE, MASTER_EDGE))
}

fn decode_raster(bytes: &[u8]) -> Result<DynamicImage, Rejected> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| Rejected::Corrupt)?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_PIXELS * 4 * 2);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|_| Rejected::Corrupt)?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(Rejected::TooManyPixels);
    }
    let orientation = decoder.orientation().map_err(|_| Rejected::Corrupt)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|_| Rejected::Corrupt)?;
    image.apply_orientation(orientation);
    Ok(image)
}

/// Draws an SVG at [`MASTER_EDGE`] on its longest edge, without fonts or linked files.
fn rasterise_svg(bytes: &[u8]) -> Result<DynamicImage, Rejected> {
    // The default resolver reads any path an `<image href>` names from the server's disk.
    let options = usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_string: Box::new(|_, _| None),
            ..usvg::ImageHrefResolver::default()
        },
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(bytes, &options).map_err(|_| Rejected::UnsupportedType)?;
    let size = tree.size();
    let scale = f64::from(MASTER_EDGE) / f64::from(size.width().max(size.height()));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "≤ 2560"
    )]
    let edge = |length: f32| ((f64::from(length) * scale).round() as u32).clamp(1, MASTER_EDGE);
    let (width, height) = (edge(size.width()), edge(size.height()));
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or(Rejected::Corrupt)?;
    #[expect(clippy::cast_possible_truncation, reason = "a scale factor")]
    let transform = tiny_skia::Transform::from_scale(scale as f32, scale as f32);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let pixels = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    RgbaImage::from_raw(width, height, pixels)
        .map(DynamicImage::ImageRgba8)
        .ok_or(Rejected::Corrupt)
}

/// Shrinks to fit `width` × `height`, keeping the aspect ratio; never enlarges.
fn fit(image: DynamicImage, width: u32, height: u32) -> DynamicImage {
    if image.width() <= width && image.height() <= height {
        image
    } else {
        image.resize(width, height, FilterType::Lanczos3)
    }
}

/// The master as stored: PNG when it has transparency, otherwise JPEG; either way without
/// metadata. Returns the bytes and the file extension.
pub(crate) fn encode_master(image: &DynamicImage) -> anyhow::Result<(Vec<u8>, &'static str)> {
    let mut bytes = Vec::new();
    if image.color().has_alpha() {
        image
            .to_rgba8()
            .write_with_encoder(PngEncoder::new(&mut bytes))?;
        Ok((bytes, "png"))
    } else {
        image
            .to_rgb8()
            .write_with_encoder(JpegEncoder::new_with_quality(&mut bytes, 90))?;
        Ok((bytes, "jpg"))
    }
}

/// Renders one [`Variant`] of a master.
pub(crate) fn render(image: &DynamicImage, variant: Variant) -> anyhow::Result<Vec<u8>> {
    let image = fit(image.clone(), variant.width(), u32::MAX);
    if variant == Variant::Share {
        let mut bytes = Vec::new();
        on_white(&image).write_with_encoder(JpegEncoder::new_with_quality(&mut bytes, 80))?;
        return Ok(bytes);
    }
    let config = zenwebp::LossyConfig::new().with_quality(75.0);
    let (pixels, layout) = if image.color().has_alpha() {
        (image.to_rgba8().into_raw(), zenwebp::PixelLayout::Rgba8)
    } else {
        (image.to_rgb8().into_raw(), zenwebp::PixelLayout::Rgb8)
    };
    zenwebp::EncodeRequest::lossy(&config, &pixels, layout, image.width(), image.height())
        .encode()
        .map_err(|error| anyhow::anyhow!("cannot encode WebP: {error}"))
}

/// Flattens transparency onto white, since JPEG has none.
fn on_white(image: &DynamicImage) -> RgbImage {
    let rgba = image.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let blend = |c: u8| {
            let mixed = (u16::from(c) * u16::from(a) + 255 * (255 - u16::from(a))) / 255;
            u8::try_from(mixed).unwrap_or(u8::MAX)
        };
        image::Rgb([blend(r), blend(g), blend(b)])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A JPEG of `width` × `height` carrying EXIF orientation 6 (rotate 90° clockwise) and a
    /// GPS marker string inside its APP1 segment.
    fn phone_jpeg(width: u32, height: u32) -> Vec<u8> {
        let mut plain = Vec::new();
        RgbImage::from_pixel(width, height, image::Rgb([200, 30, 30]))
            .write_with_encoder(JpegEncoder::new(&mut plain))
            .unwrap();
        // Big-endian TIFF: one IFD entry, Orientation (0x0112), SHORT, 1, value 6.
        let mut exif =
            b"Exif\0\0MM\0\x2a\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01\0\x06\0\0\0\0\0\0".to_vec();
        exif.extend_from_slice(b"GPS -6.2000 106.8166");
        let length = u16::try_from(exif.len() + 2).unwrap().to_be_bytes();
        let mut jpeg = plain[..2].to_vec();
        jpeg.extend_from_slice(&[0xFF, 0xE1, length[0], length[1]]);
        jpeg.extend_from_slice(&exif);
        jpeg.extend_from_slice(&plain[2..]);
        jpeg
    }

    fn png(image: &DynamicImage) -> Vec<u8> {
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn exif_orientation_is_applied_and_every_output_drops_the_metadata() {
        let upload = phone_jpeg(40, 20);
        assert!(contains(&upload, b"GPS"));

        let image = decode(Kind::Photo, &upload).unwrap();

        assert_eq!((image.width(), image.height()), (20, 40));
        let mut outputs = vec![encode_master(&image).unwrap().0];
        outputs.extend(Variant::ALL.map(|variant| render(&image, variant).unwrap()));
        for output in outputs {
            assert!(!contains(&output, b"Exif") && !contains(&output, b"GPS"));
        }
    }

    #[test]
    fn more_than_40_megapixels_is_refused_before_decoding() {
        let huge = DynamicImage::ImageLuma8(image::GrayImage::new(8000, 5001));

        assert_eq!(
            decode(Kind::Photo, &png(&huge)),
            Err(Rejected::TooManyPixels)
        );
    }

    #[test]
    fn the_master_is_at_most_2560_px_and_each_variant_has_its_width() {
        let wide = DynamicImage::ImageRgb8(RgbImage::new(3000, 1500));

        let image = decode(Kind::Photo, &png(&wide)).unwrap();

        assert_eq!((image.width(), image.height()), (2560, 1280));
        for (variant, width, format) in [
            (Variant::W480, 480, ImageFormat::WebP),
            (Variant::W960, 960, ImageFormat::WebP),
            (Variant::W1600, 1600, ImageFormat::WebP),
            (Variant::W2400, 2400, ImageFormat::WebP),
            (Variant::Share, 1200, ImageFormat::Jpeg),
        ] {
            let bytes = render(&image, variant).unwrap();
            assert_eq!(image::guess_format(&bytes).unwrap(), format, "{variant:?}");
            let rendered = image::load_from_memory(&bytes).unwrap();
            assert_eq!((rendered.width(), rendered.height()), (width, width / 2));
        }
    }

    #[test]
    fn a_small_image_is_never_enlarged() {
        let small = DynamicImage::ImageRgb8(RgbImage::new(300, 200));
        let image = decode(Kind::Photo, &png(&small)).unwrap();

        let rendered = image::load_from_memory(&render(&image, Variant::W1600).unwrap()).unwrap();

        assert_eq!(rendered.width(), 300);
    }

    #[test]
    fn the_type_comes_from_the_content_and_each_kind_takes_its_own() {
        let jpeg = phone_jpeg(4, 4);
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 10"><rect width="20" height="10" fill="red"/></svg>"#;

        assert!(decode(Kind::Photo, &jpeg).is_ok());
        assert_eq!(decode(Kind::Logo, &jpeg), Err(Rejected::UnsupportedType));
        assert_eq!(decode(Kind::Photo, svg), Err(Rejected::UnsupportedType));
        assert_eq!(
            decode(Kind::Logo, b"%PDF-1.7"),
            Err(Rejected::UnsupportedType)
        );
        assert_eq!(decode(Kind::Photo, &jpeg[..40]), Err(Rejected::Corrupt));
    }

    #[test]
    fn an_svg_logo_is_rasterised_with_its_transparency() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 10"><rect width="10" height="10" fill="red"/></svg>"#;

        let image = decode(Kind::Logo, svg).unwrap();

        assert_eq!((image.width(), image.height()), (2560, 1280));
        let rgba = image.to_rgba8();
        assert_eq!(rgba.get_pixel(10, 10).0, [255, 0, 0, 255]);
        assert_eq!(rgba.get_pixel(2550, 10).0[3], 0);
        assert_eq!(encode_master(&image).unwrap().1, "png");
        let share = image::load_from_memory(&render(&image, Variant::Share).unwrap()).unwrap();
        assert!(
            share
                .to_rgb8()
                .get_pixel(1190, 10)
                .0
                .iter()
                .all(|&c| c > 250)
        );
    }

    #[test]
    fn an_svg_logo_cannot_pull_in_files_from_the_server() {
        let dir = tempfile::tempdir().unwrap();
        let local = dir.path().join("secret.svg");
        std::fs::write(
            &local,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><rect width="1" height="1" fill="red"/></svg>"#,
        )
        .unwrap();
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><image href="{}" width="10" height="10"/></svg>"#,
            local.display()
        );

        let image = decode(Kind::Logo, svg.as_bytes()).unwrap();

        assert_eq!(image.to_rgba8().get_pixel(1280, 1280).0[3], 0);
    }
}
