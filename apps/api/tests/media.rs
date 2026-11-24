mod common;

use std::{io::Cursor, path::PathBuf};

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use common::test_state;
use http_body_util::BodyExt;
use image::{ImageBuffer, Rgb, codecs::jpeg::JpegEncoder};
use pranata::{ApiError, AppState, auth, jobs, media::pipeline};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

const PASSWORD: &str = "correct horse battery";
const BOUNDARY: &str = "pranata-test-boundary";

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// A plain gradient JPEG of the given width, with no metadata.
fn jpeg(width: u32, height: u32) -> Vec<u8> {
    let image = ImageBuffer::from_fn(width, height, |x, y| {
        Rgb([(x % 251) as u8, (y % 241) as u8, 90])
    });
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 85)
        .encode_image(&image)
        .unwrap();
    out
}

fn multipart(file_name: &str, file: &[u8], kind: Option<&str>) -> Vec<u8> {
    let mut body = Vec::new();
    if let Some(kind) = kind {
        body.extend(
            format!(
                "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"kind\"\r\n\r\n{kind}\r\n"
            )
            .into_bytes(),
        );
    }
    body.extend(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .into_bytes(),
    );
    body.extend(file);
    body.extend(format!("\r\n--{BOUNDARY}--\r\n").into_bytes());
    body
}

struct Reply {
    status: StatusCode,
    #[allow(dead_code)]
    headers: HeaderMap,
    body: Value,
}

struct Harness {
    pool: PgPool,
    state: AppState,
    app: Router,
    cookie: String,
    csrf: String,
}

impl Harness {
    async fn new(pool: PgPool) -> Self {
        let state = test_state(pool.clone());
        let app = pranata::app(state.clone());
        let hash = auth::password::hash_password(PASSWORD).unwrap();
        sqlx::query!(
            "INSERT INTO users (username, display_name, password_hash)
             VALUES ('budi', 'Budi Santoso', $1)",
            hash
        )
        .execute(&pool)
        .await
        .unwrap();
        let login = Request::builder()
            .method(Method::POST)
            .uri("/api/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"username": "budi", "password": PASSWORD}).to_string(),
            ))
            .unwrap();
        let response = app.clone().oneshot(login).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let cookie = response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let csrf = serde_json::from_slice::<Value>(&bytes).unwrap()["csrf_token"]
            .as_str()
            .unwrap()
            .to_owned();
        Self {
            pool,
            state,
            app,
            cookie,
            csrf,
        }
    }

    async fn send(&self, request: Request<Body>) -> Reply {
        let response = self.app.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        Reply {
            status: parts.status,
            headers: parts.headers,
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        }
    }

    async fn upload_with(&self, body: Vec<u8>, signed_in: bool, csrf: Option<&str>) -> Reply {
        let mut builder = Request::builder()
            .method(Method::POST)
            .uri("/api/admin/media")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={BOUNDARY}"),
            );
        if signed_in {
            builder = builder.header(header::COOKIE, &self.cookie);
        }
        if let Some(csrf) = csrf {
            builder = builder.header("x-csrf-token", csrf);
        }
        self.send(builder.body(Body::from(body)).unwrap()).await
    }

    async fn upload(&self, name: &str, file: &[u8], kind: Option<&str>) -> Reply {
        self.upload_with(multipart(name, file, kind), true, Some(&self.csrf))
            .await
    }

    async fn get(&self, id: &str) -> Reply {
        self.send(
            Request::builder()
                .uri(format!("/api/admin/media/{id}"))
                .header(header::COOKIE, &self.cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }

    /// Run every due job, as the worker would.
    async fn run_jobs(&self) {
        while jobs::claim_and_run(&self.state).await.unwrap() {}
    }

    async fn stored(&self, rel: &str) -> Vec<u8> {
        self.state.storage.get(rel).await.unwrap().to_vec()
    }
}

fn hex_of(sha: &[u8]) -> String {
    pranata::media::hex(sha)
}

fn has_exif(bytes: &[u8]) -> bool {
    exif::Reader::new()
        .read_from_container(&mut Cursor::new(bytes))
        .is_ok()
}

fn widths(body: &Value) -> Vec<u64> {
    body["variants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|variant| variant["width"].as_u64().unwrap())
        .collect()
}

#[sqlx::test]
async fn gps_exif_is_stripped(pool: PgPool) {
    let raw = fixture("gps.jpg");
    assert!(has_exif(&raw), "the fixture must carry an EXIF block");
    let h = Harness::new(pool).await;

    let created = h.upload("IMG-001.jpg", &raw, None).await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    assert_eq!(created.body["status"], "pending");
    h.run_jobs().await;

    let asset = h.get(created.body["id"].as_str().unwrap()).await;
    assert_eq!(asset.status, StatusCode::OK);
    assert_eq!(asset.body["status"], "ready");

    let row = sqlx::query!("SELECT sha256 FROM media_assets")
        .fetch_one(&h.pool)
        .await
        .unwrap();
    let hex = hex_of(&row.sha256);
    let original = h
        .stored(&format!("originals/{}/{hex}.jpg", &hex[..2]))
        .await;
    assert!(!has_exif(&original), "stored original still has EXIF");
    assert!(!original.windows(6).any(|w| w == b"Exif\0\0"));
    // The original is still a JPEG.
    assert_eq!(&original[..3], &[0xFF, 0xD8, 0xFF]);

    let variants = asset.body["variants"].as_array().unwrap();
    assert_eq!(variants.len(), 1, "a 64 px image has only its own width");
    for variant in variants {
        let url = variant["url"].as_str().unwrap();
        let rel = format!("public/{}", url.strip_prefix("/media/").unwrap());
        let bytes = h.stored(&rel).await;
        assert!(!has_exif(&bytes), "{url} still has EXIF");
        assert!(
            !bytes.windows(4).any(|w| w == b"EXIF"),
            "{url} has an EXIF chunk"
        );
        assert!(
            !bytes.windows(4).any(|w| w == b"ICCP"),
            "{url} has an ICC chunk"
        );
    }
}

#[sqlx::test]
async fn orientation_6_is_applied(pool: PgPool) {
    let raw = fixture("rotated-6.jpg");
    let (raw_w, raw_h) = image::ImageReader::new(Cursor::new(&raw))
        .with_guessed_format()
        .unwrap()
        .into_dimensions()
        .unwrap();
    assert_eq!((raw_w, raw_h), (80, 40));
    let h = Harness::new(pool).await;

    let created = h.upload("rotated.jpg", &raw, None).await;
    assert_eq!(created.status, StatusCode::CREATED);
    h.run_jobs().await;
    let asset = h.get(created.body["id"].as_str().unwrap()).await;

    // Width and height are swapped compared with the raw header.
    assert_eq!(asset.body["width"], raw_h);
    assert_eq!(asset.body["height"], raw_w);
    let url = asset.body["variants"][0]["url"].as_str().unwrap();
    let variant = h
        .stored(&format!("public/{}", url.strip_prefix("/media/").unwrap()))
        .await;
    let (w, v_h) = image::ImageReader::new(Cursor::new(&variant))
        .with_guessed_format()
        .unwrap()
        .into_dimensions()
        .unwrap();
    assert_eq!((w, v_h), (40, 80));
}

#[sqlx::test]
async fn variants_never_upscale(pool: PgPool) {
    let h = Harness::new(pool).await;
    let created = h.upload("wide.jpg", &jpeg(1000, 600), None).await;
    assert_eq!(created.status, StatusCode::CREATED);
    h.run_jobs().await;
    let asset = h.get(created.body["id"].as_str().unwrap()).await;

    // 1600 and 2560 would upscale; the image's own width keeps the largest
    // public image sharp.
    assert_eq!(widths(&asset.body), [480, 960, 1000]);

    let row = sqlx::query!("SELECT sha256 FROM media_assets")
        .fetch_one(&h.pool)
        .await
        .unwrap();
    let hex = hex_of(&row.sha256);
    for variant in asset.body["variants"].as_array().unwrap() {
        let width = variant["width"].as_u64().unwrap();
        assert_eq!(
            variant["url"],
            format!("/media/{}/{hex}-{width}.webp", &hex[..2])
        );
        let bytes = h
            .stored(&format!("public/{}/{hex}-{width}.webp", &hex[..2]))
            .await;
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WEBP");
    }
    // Originals are private.
    let root = h.state.config.media_dir.clone();
    assert!(
        root.join(format!("originals/{}/{hex}.jpg", &hex[..2]))
            .exists()
    );
    for entry in walk(&root.join("public")) {
        assert_eq!(entry.extension().unwrap(), "webp");
    }
}

fn walk(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else {
            files.push(path);
        }
    }
    files
}

#[sqlx::test]
async fn large_images_get_every_width(pool: PgPool) {
    let h = Harness::new(pool).await;
    let created = h.upload("big.jpg", &jpeg(1700, 40), None).await;
    h.run_jobs().await;
    let asset = h.get(created.body["id"].as_str().unwrap()).await;
    assert_eq!(widths(&asset.body), [480, 960, 1600, 1700]);
}

#[sqlx::test]
async fn heic_is_415(pool: PgPool) {
    let h = Harness::new(pool).await;
    let reply = h.upload("IMG.heic", &fixture("fake.heic"), None).await;
    assert_eq!(reply.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(reply.body["code"], "unsupported_media_type");

    // Kind and content must agree.
    let pdf = fixture("doc.pdf");
    let reply = h.upload("a.pdf", &pdf, Some("image")).await;
    assert_eq!(reply.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let reply = h.upload("a.jpg", &jpeg(50, 50), Some("document")).await;
    assert_eq!(reply.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let rows = sqlx::query_scalar!("SELECT count(*) AS \"n!\" FROM media_assets")
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

#[sqlx::test]
async fn bomb_is_rejected_before_decode(pool: PgPool) {
    let bomb = fixture("bomb.png");
    assert!(bomb.len() < 200);

    // The pipeline refuses on the header alone.
    let error = pipeline::process_image(&bomb).unwrap_err();
    assert!(
        matches!(error, ApiError::Unprocessable("image_too_large")),
        "{error:?}"
    );

    let h = Harness::new(pool).await;
    let reply = h.upload("bomb.png", &bomb, None).await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.body["code"], "image_too_large");
    let assets = sqlx::query_scalar!("SELECT count(*) AS \"n!\" FROM media_assets")
        .fetch_one(&h.pool)
        .await
        .unwrap();
    let queued = sqlx::query_scalar!("SELECT count(*) AS \"n!\" FROM jobs")
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert_eq!((assets, queued), (0, 0));
}

#[sqlx::test]
async fn same_file_twice_returns_same_asset(pool: PgPool) {
    let h = Harness::new(pool).await;
    let file = jpeg(300, 200);
    let first = h.upload("one.jpg", &file, None).await;
    let second = h.upload("copy-of-one.jpg", &file, None).await;
    assert_eq!(first.status, StatusCode::CREATED);
    assert_eq!(second.status, StatusCode::OK);
    assert_eq!(first.body["id"], second.body["id"]);

    let assets = sqlx::query_scalar!("SELECT count(*) AS \"n!\" FROM media_assets")
        .fetch_one(&h.pool)
        .await
        .unwrap();
    let queued = sqlx::query_scalar!("SELECT count(*) AS \"n!\" FROM jobs")
        .fetch_one(&h.pool)
        .await
        .unwrap();
    let audited = sqlx::query_scalar!(
        "SELECT count(*) AS \"n!\" FROM audit_entries WHERE object_type = 'media_asset'"
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!((assets, queued, audited), (1, 1, 1));
}

#[sqlx::test]
async fn upload_writes_audit_entry_and_job_together(pool: PgPool) {
    let h = Harness::new(pool).await;
    let created = h.upload("kegiatan.jpg", &jpeg(200, 100), None).await;
    let id = created.body["id"].as_str().unwrap();
    let entry = sqlx::query!(
        "SELECT action, object_type, object_id, actor FROM audit_entries
         WHERE object_type = 'media_asset'"
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(entry.action, "create");
    assert_eq!(entry.object_id, id);
    assert!(entry.actor.is_some());
    let job = sqlx::query!("SELECT kind, payload FROM jobs")
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert_eq!(job.kind, "process_image");
    assert_eq!(job.payload["asset_id"], id);
}

#[sqlx::test]
async fn oversize_upload_is_413(pool: PgPool) {
    let h = Harness::new(pool).await;
    // Images stop at 10 MB.
    let mut image = vec![0xFF, 0xD8, 0xFF];
    image.resize(10 * 1024 * 1024 + 1, 0);
    let reply = h.upload("huge.jpg", &image, None).await;
    assert_eq!(reply.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(reply.body["code"], "file_too_large");

    // PDFs stop at 20 MB.
    let mut pdf = b"%PDF-1.4\n".to_vec();
    pdf.resize(20 * 1024 * 1024 + 1, b' ');
    let reply = h.upload("huge.pdf", &pdf, Some("document")).await;
    assert_eq!(reply.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(reply.body["code"], "file_too_large");

    // Far beyond any limit, the request body limit answers the same way.
    let mut huge = vec![0xFF, 0xD8, 0xFF];
    huge.resize(25 * 1024 * 1024, 0);
    let reply = h.upload("huge.jpg", &huge, None).await;
    assert_eq!(reply.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(reply.body["code"], "file_too_large");
}

#[sqlx::test]
async fn documents_are_ready_at_once_and_get_no_job(pool: PgPool) {
    let h = Harness::new(pool).await;
    let reply = h
        .upload("laporan.pdf", &fixture("doc.pdf"), Some("document"))
        .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.body);
    assert_eq!(reply.body["kind"], "document");
    assert_eq!(reply.body["status"], "ready");
    assert_eq!(reply.body["variants"], json!([]));
    let queued = sqlx::query_scalar!("SELECT count(*) AS \"n!\" FROM jobs")
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert_eq!(queued, 0);
}

#[sqlx::test]
async fn undecodable_image_is_marked_failed_without_retries(pool: PgPool) {
    let h = Harness::new(pool).await;
    // The header is intact, the pixel data is cut off.
    let image = ImageBuffer::from_fn(300, 200, |x, y| {
        Rgb([(x * y % 251) as u8, 7, (x % 13) as u8])
    });
    let mut file = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut file), image::ImageFormat::Png)
        .unwrap();
    file.truncate(file.len() / 2);
    let created = h.upload("cut.png", &file, None).await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    h.run_jobs().await;
    let asset = h.get(created.body["id"].as_str().unwrap()).await;
    assert_eq!(asset.body["status"], "failed");
    let job = sqlx::query!("SELECT done_at, attempts FROM jobs")
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert!(job.done_at.is_some());
    assert_eq!(job.attempts, 1, "one attempt, no retries");
}

#[sqlx::test]
async fn media_routes_need_a_session_and_csrf(pool: PgPool) {
    let h = Harness::new(pool).await;
    let body = multipart("a.jpg", &jpeg(50, 50), None);

    let anonymous = h.upload_with(body.clone(), false, None).await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);

    let no_csrf = h.upload_with(body.clone(), true, None).await;
    assert_eq!(no_csrf.status, StatusCode::FORBIDDEN);
    assert_eq!(no_csrf.body["code"], "csrf_mismatch");

    let read = h
        .send(
            Request::builder()
                .uri(format!("/api/admin/media/{}", uuid::Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(read.status, StatusCode::UNAUTHORIZED);

    let missing = h.get(&uuid::Uuid::new_v4().to_string()).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.body["code"], "media_not_found");
}

#[sqlx::test]
async fn upload_needs_a_file_field(pool: PgPool) {
    let h = Harness::new(pool).await;
    let body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"kind\"\r\n\r\nimage\r\n--{BOUNDARY}--\r\n"
    )
    .into_bytes();
    let reply = h.upload_with(body, true, Some(&h.csrf)).await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.body["errors"][0]["field"], "file");
}

/// Upload an image, lose its stored original, and run its job until it gives up.
async fn exhaust_job(h: &Harness, file: &[u8]) -> Value {
    let created = h.upload("kegiatan.jpg", file, None).await;
    assert_eq!(created.status, StatusCode::CREATED);
    let row = sqlx::query!("SELECT sha256 FROM media_assets")
        .fetch_one(&h.pool)
        .await
        .unwrap();
    let hex = hex_of(&row.sha256);
    std::fs::remove_file(
        h.state
            .config
            .media_dir
            .join(format!("originals/{}/{hex}.jpg", &hex[..2])),
    )
    .unwrap();
    for _ in 0..jobs::MAX_ATTEMPTS {
        sqlx::query!("UPDATE jobs SET run_at = now()")
            .execute(&h.pool)
            .await
            .unwrap();
        assert!(jobs::claim_and_run(&h.state).await.unwrap());
    }
    created.body
}

#[sqlx::test]
async fn exhausted_job_marks_asset_failed(pool: PgPool) {
    let h = Harness::new(pool).await;
    let body = exhaust_job(&h, &jpeg(300, 200)).await;
    let job = sqlx::query!("SELECT failed_at FROM jobs")
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert!(job.failed_at.is_some());
    let asset = h.get(body["id"].as_str().unwrap()).await;
    assert_eq!(asset.body["status"], "failed");
}

#[sqlx::test]
async fn reupload_of_failed_asset_retries(pool: PgPool) {
    let h = Harness::new(pool).await;
    let file = jpeg(300, 200);
    let body = exhaust_job(&h, &file).await;

    let again = h.upload("kegiatan.jpg", &file, None).await;
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(again.body["id"], body["id"]);
    assert_eq!(again.body["status"], "pending");
    let open = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM jobs WHERE done_at IS NULL AND failed_at IS NULL"#
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(open, 1, "a fresh job is queued");

    h.run_jobs().await;
    let asset = h.get(body["id"].as_str().unwrap()).await;
    assert_eq!(asset.body["status"], "ready");
    let assets = sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM media_assets"#)
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert_eq!(assets, 1);
}
