//! Regenerates the media test fixtures in `tests/fixtures/`.
//!
//! Run from the repository root with:
//!
//! ```text
//! cargo run -p pranata --example make_fixtures
//! ```
//!
//! Every file is synthetic (gradients and hand-written bytes), so no real
//! photo, person or place is involved. The GPS position is a made-up point.
//!
//! - `gps.jpg`: 64x48 JPEG with an EXIF block that holds a GPS position.
//! - `rotated-6.jpg`: 80x40 JPEG with EXIF orientation 6 (rotate 90 degrees
//!   clockwise), so it must come out 40x80.
//! - `bomb.png`: a few dozen bytes whose header declares 30000x30000 pixels.
//! - `fake.heic`: HEIC magic bytes only (an `ftyp` box with brand `heic`).
//! - `doc.pdf`: a minimal one-page PDF.
//! - `apps/web/e2e/fixtures/gps-photo.jpg`: 3264x2448 (8 MP) JPEG with the same
//!   GPS EXIF block, standing in for a phone photo in the end-to-end tests.

use std::{fs, io::Cursor, path::Path};

use exif::{Field, In, Rational, Tag, Value, experimental::Writer};
use flate2::{Compression, Crc, write::ZlibEncoder};
use image::{ImageBuffer, Rgb, codecs::jpeg::JpegEncoder};

fn gradient_jpeg(width: u32, height: u32) -> Vec<u8> {
    let image = ImageBuffer::from_fn(width, height, |x, y| {
        Rgb([
            u8::try_from(x * 255 / width).unwrap_or(255),
            u8::try_from(y * 255 / height).unwrap_or(255),
            128,
        ])
    });
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 90)
        .encode_image(&image)
        .expect("encode jpeg");
    out
}

/// Put an EXIF APP1 segment right after the JPEG start-of-image marker.
fn with_exif(jpeg: &[u8], fields: &[Field]) -> Vec<u8> {
    let mut writer = Writer::new();
    for field in fields {
        writer.push_field(field);
    }
    let mut tiff = Cursor::new(Vec::new());
    writer.write(&mut tiff, false).expect("write exif");
    let mut payload = b"Exif\0\0".to_vec();
    payload.extend(tiff.into_inner());
    let length = u16::try_from(payload.len() + 2).expect("exif block fits one segment");

    let mut out = jpeg[..2].to_vec();
    out.extend([0xFF, 0xE1]);
    out.extend(length.to_be_bytes());
    out.extend(payload);
    out.extend(&jpeg[2..]);
    out
}

fn gps_fields() -> Vec<Field> {
    let dms = |degrees, minutes, seconds| {
        Value::Rational(vec![
            Rational {
                num: degrees,
                denom: 1,
            },
            Rational {
                num: minutes,
                denom: 1,
            },
            Rational {
                num: seconds,
                denom: 1,
            },
        ])
    };
    let ascii = |text: &[u8]| Value::Ascii(vec![text.to_vec()]);
    let field = |tag, value| Field {
        tag,
        ifd_num: In::PRIMARY,
        value,
    };
    vec![
        field(Tag::Make, ascii(b"Example Phone")),
        field(Tag::GPSLatitudeRef, ascii(b"S")),
        field(Tag::GPSLatitude, dms(6, 10, 30)),
        field(Tag::GPSLongitudeRef, ascii(b"E")),
        field(Tag::GPSLongitude, dms(106, 49, 40)),
    ]
}

fn png_chunk(kind: [u8; 4], data: &[u8]) -> Vec<u8> {
    let mut crc = Crc::new();
    crc.update(&kind);
    crc.update(data);
    let mut out = u32::try_from(data.len())
        .expect("small chunk")
        .to_be_bytes()
        .to_vec();
    out.extend(kind);
    out.extend(data);
    out.extend(crc.sum().to_be_bytes());
    out
}

/// A PNG whose header says 30000x30000 but whose pixel data is a single row.
fn bomb_png() -> Vec<u8> {
    use std::io::Write;

    let mut header = Vec::new();
    header.extend(30_000_u32.to_be_bytes());
    header.extend(30_000_u32.to_be_bytes());
    header.extend([8, 2, 0, 0, 0]); // 8-bit RGB, no interlace
    let mut zlib = ZlibEncoder::new(Vec::new(), Compression::best());
    zlib.write_all(&[0; 16]).expect("compress");
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    out.extend(png_chunk(*b"IHDR", &header));
    out.extend(png_chunk(*b"IDAT", &zlib.finish().expect("compress")));
    out.extend(png_chunk(*b"IEND", &[]));
    out
}

fn fake_heic() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(24_u32.to_be_bytes());
    out.extend(b"ftypheic");
    out.extend(0_u32.to_be_bytes());
    out.extend(b"mif1heic");
    out
}

const PDF: &[u8] = b"%PDF-1.4\n\
1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n\
trailer\n<< /Root 1 0 R /Size 4 >>\n%%EOF\n";

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    fs::create_dir_all(&dir).expect("create fixtures dir");
    let rotated = [Field {
        tag: Tag::Orientation,
        ifd_num: In::PRIMARY,
        value: Value::Short(vec![6]),
    }];
    let files: [(&str, Vec<u8>); 5] = [
        ("gps.jpg", with_exif(&gradient_jpeg(64, 48), &gps_fields())),
        ("rotated-6.jpg", with_exif(&gradient_jpeg(80, 40), &rotated)),
        ("bomb.png", bomb_png()),
        ("fake.heic", fake_heic()),
        ("doc.pdf", PDF.to_vec()),
    ];
    for (name, bytes) in files {
        fs::write(dir.join(name), &bytes).expect("write fixture");
        println!("{name}: {} bytes", bytes.len());
    }

    let e2e = Path::new(env!("CARGO_MANIFEST_DIR")).join("../web/e2e/fixtures");
    fs::create_dir_all(&e2e).expect("create e2e fixtures dir");
    let photo = with_exif(&gradient_jpeg(3264, 2448), &gps_fields());
    fs::write(e2e.join("gps-photo.jpg"), &photo).expect("write e2e photo");
    println!("gps-photo.jpg: {} bytes", photo.len());
}
