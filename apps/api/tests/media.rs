mod common;

use std::io::Cursor;

use axum::http::{StatusCode, header};
use common::TestApp;
use image::{DynamicImage, ImageFormat, RgbaImage};
use sqlx::PgPool;

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        width,
        height,
        image::Rgba([0, 90, 60, 255]),
    ))
    .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
    .unwrap();
    bytes
}

const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><circle cx="5" cy="5" r="5"/></svg>"#;

#[sqlx::test]
async fn an_uploaded_logo_is_served_as_immutable_variants(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "operator", &["village.manage"]).await;

    let reply = app.upload("/village/logo", png(800, 400), &cookie).await;

    assert_eq!(reply.status, StatusCode::CREATED, "{:?}", reply.body);
    let id = reply.body["id"].as_str().unwrap();
    for (variant, content_type, format, width) in [
        ("480", "image/webp", ImageFormat::WebP, 480),
        ("960", "image/webp", ImageFormat::WebP, 800),
        ("1600", "image/webp", ImageFormat::WebP, 800),
        ("share", "image/jpeg", ImageFormat::Jpeg, 800),
    ] {
        let (status, headers, bytes) = app.get_raw(&format!("/media/{id}/{variant}")).await;
        assert_eq!(status, StatusCode::OK, "{variant}");
        assert_eq!(headers[header::CONTENT_TYPE], content_type);
        assert_eq!(
            headers[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );
        let image = image::load_from_memory_with_format(&bytes, format).unwrap();
        assert_eq!(image.width(), width, "{variant}");
    }
}

#[sqlx::test]
async fn an_svg_logo_is_rasterised_and_never_kept(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "operator", &["village.manage"]).await;

    let reply = app.upload("/village/logo", SVG.into(), &cookie).await;

    assert_eq!(reply.status, StatusCode::CREATED);
    let id = reply.body["id"].as_str().unwrap();
    let (status, _, _) = app.get_raw(&format!("/media/{id}/480")).await;
    assert_eq!(status, StatusCode::OK);
    let mut kept: Vec<_> = std::fs::read_dir(app.files.path().join(id))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    kept.sort();
    assert_eq!(
        kept,
        [
            "1600.webp",
            "480.webp",
            "960.webp",
            "master.png",
            "share.jpg"
        ]
    );
}

#[sqlx::test]
async fn a_logo_must_be_a_png_or_svg_of_at_most_2_mib(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "operator", &["village.manage"]).await;
    let mut jpeg = Vec::new();
    DynamicImage::new_rgb8(8, 8)
        .write_to(&mut Cursor::new(&mut jpeg), ImageFormat::Jpeg)
        .unwrap();
    let mut oversized = png(8, 8);
    oversized.resize((2 << 20) + 1, 0);

    for (upload, expected) in [
        (jpeg, StatusCode::UNSUPPORTED_MEDIA_TYPE),
        (b"not an image".to_vec(), StatusCode::UNSUPPORTED_MEDIA_TYPE),
        (png(8, 8)[..60].to_vec(), StatusCode::UNPROCESSABLE_ENTITY),
        (oversized, StatusCode::PAYLOAD_TOO_LARGE),
    ] {
        let reply = app.upload("/village/logo", upload, &cookie).await;
        assert_eq!(reply.status, expected);
    }
}

#[sqlx::test]
async fn only_village_managers_upload_a_logo(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "writer", &["content.write"]).await;

    let refused = app.upload("/village/logo", png(8, 8), &cookie).await;
    let anonymous = app.upload("/village/logo", png(8, 8), "x=y").await;

    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn unknown_files_and_variants_are_not_found(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "operator", &["village.manage"]).await;
    let id = app.upload("/village/logo", png(8, 8), &cookie).await.body["id"]
        .as_str()
        .unwrap()
        .to_owned();

    for path in [
        format!("/media/{id}/master"),
        format!("/media/{id}/2400"),
        format!("/media/{}/480", uuid::Uuid::nil()),
    ] {
        let (status, _, _) = app.get_raw(&path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}

async fn upload_logo(app: &TestApp, cookie: &str) -> String {
    let reply = app.upload("/village/logo", png(8, 8), cookie).await;
    reply.body["id"].as_str().unwrap().to_owned()
}

async fn served(app: &TestApp, id: &str) -> bool {
    app.get_raw(&format!("/media/{id}/480")).await.0 == StatusCode::OK
}

/// Moves a file's sweep deadline into the past, as if a day had gone by.
async fn age(pool: &PgPool, id: &str) {
    sqlx::query("UPDATE files SET sweep_after = now() - interval '1 second' WHERE id = $1::uuid")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}

async fn save_logo(app: &TestApp, id: &str, cookie: &str) {
    let body = serde_json::json!({ "logo": id });
    let reply = app
        .call("PUT", "/village/profile", Some(&body), Some(cookie))
        .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
}

#[sqlx::test]
async fn an_upload_no_owner_references_is_swept_after_24_hours(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "operator", &["village.manage"]).await;
    let (fresh, stale) = (
        upload_logo(&app, &cookie).await,
        upload_logo(&app, &cookie).await,
    );
    age(&pool, &stale).await;

    let swept = pranata::storage::sweep(&pool, app.files.path())
        .await
        .unwrap();

    assert_eq!(swept, 1);
    assert!(served(&app, &fresh).await);
    assert!(!served(&app, &stale).await);
    assert!(!app.files.path().join(&stale).exists());
}

#[sqlx::test]
async fn a_logo_in_use_is_kept_and_a_replaced_one_gets_another_24_hours(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "operator", &["village.manage"]).await;
    let old = upload_logo(&app, &cookie).await;
    save_logo(&app, &old, &cookie).await;
    age(&pool, &old).await;
    pranata::storage::sweep(&pool, app.files.path())
        .await
        .unwrap();
    assert!(served(&app, &old).await);

    let new = upload_logo(&app, &cookie).await;
    save_logo(&app, &new, &cookie).await;
    pranata::storage::sweep(&pool, app.files.path())
        .await
        .unwrap();
    assert!(served(&app, &old).await);

    age(&pool, &old).await;
    age(&pool, &new).await;
    pranata::storage::sweep(&pool, app.files.path())
        .await
        .unwrap();
    assert!(!served(&app, &old).await);
    assert!(served(&app, &new).await);
}

#[sqlx::test]
async fn regenerating_rebuilds_every_variant_from_the_master(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "operator", &["village.manage"]).await;
    let id = upload_logo(&app, &cookie).await;
    let file = app.files.path().join(&id);
    std::fs::remove_file(file.join("480.webp")).unwrap();
    std::fs::write(file.join("share.jpg"), b"broken").unwrap();

    let rebuilt = pranata::storage::regenerate(app.files.path()).unwrap();

    assert_eq!(rebuilt, 1);
    let (status, _, bytes) = app.get_raw(&format!("/media/{id}/480")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(image::load_from_memory_with_format(&bytes, ImageFormat::WebP).is_ok());
    let (_, _, bytes) = app.get_raw(&format!("/media/{id}/share")).await;
    assert!(image::load_from_memory_with_format(&bytes, ImageFormat::Jpeg).is_ok());
}
