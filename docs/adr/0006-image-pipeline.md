# Downscale photos in the browser, re-encode on the server, strip metadata

Photos are downscaled in the browser before upload (long side at most 2560 px). A background job on the server then auto-rotates by EXIF orientation, strips every metadata field, produces width variants and encodes lossy WebP through libwebp.

Village staff post from phones over weak signals, and most photos arrive as multi-megabyte WhatsApp or camera files; downscaling first turns an 8 MB upload into about 1 MB. Phone photos carry GPS coordinates, often of residents' homes, which must never reach the public site. The `image` crate alone writes only lossless WebP, which is too large for public pages.

## Considered Options

- **Server-only processing of full-size uploads**: simplest client, but slow or failing uploads on weak signals.
- **AVIF via a pure-Rust encoder**: smaller files, but much slower to encode on a small VPS.
- **Lossless WebP from `image`**: no C dependency, but files several times larger.

## Consequences

The API image depends on libwebp (a C library); `unsafe_code = "forbid"` still applies to Pranata's own crates. Originals are kept after metadata stripping, so variants can be regenerated.
