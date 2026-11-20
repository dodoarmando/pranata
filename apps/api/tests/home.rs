mod common;

use axum::http::StatusCode;
use common::{Reply, TestApp};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A Village with a writer (`content.write`), a publisher (`content.publish`) and a hero photo
/// the publisher uploaded.
struct Office {
    app: TestApp,
    writer: String,
    publisher: String,
    photo: Value,
}

async fn office(pool: &PgPool) -> Office {
    pranata::village::set_code(pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let writer = app.user_with(pool, "penulis", &["content.write"]).await;
    let publisher = app.user_with(pool, "redaksi", &["content.publish"]).await;
    let upload = app
        .upload("/home/hero/photo", common::png(3000, 1500), &publisher)
        .await;
    assert_eq!(upload.status, StatusCode::CREATED);
    Office {
        app,
        writer,
        publisher,
        photo: upload.body["id"].clone(),
    }
}

impl Office {
    async fn call(&self, method: &str, path: &str, body: Option<&Value>, cookie: &str) -> Reply {
        self.app.call(method, path, body, Some(cookie)).await
    }

    fn hero(&self) -> Value {
        json!({
            "photo": self.photo, "photoAlt": " Sawah di kaki bukit ", "title": " Desa Sejahtera ",
            "subtitle": " ", "buttonLabel": "Profil desa", "buttonPath": "/profil",
            "quickLinks": [
                { "label": " Berita ", "path": "/berita" },
                { "label": "APBDes", "path": "/apbdes?tahun=2026" },
            ],
        })
    }
}

#[sqlx::test]
async fn a_publisher_saves_the_hero_live_and_the_public_reads_it(pool: PgPool) {
    let office = office(&pool).await;
    assert_eq!(office.app.get("/home/hero").await.0, StatusCode::NOT_FOUND);

    let saved = office
        .call("PUT", "/home/hero", Some(&office.hero()), &office.publisher)
        .await;

    assert_eq!(saved.status, StatusCode::NO_CONTENT);
    assert_eq!(
        office.app.get("/home/hero").await,
        (
            StatusCode::OK,
            json!({
                "photo": office.photo, "photoAlt": "Sawah di kaki bukit", "title": "Desa Sejahtera",
                "subtitle": null, "buttonLabel": "Profil desa", "buttonPath": "/profil",
                "quickLinks": [
                    { "label": "Berita", "path": "/berita" },
                    { "label": "APBDes", "path": "/apbdes?tahun=2026" },
                ],
            })
        )
    );
    let (status, _, _) = office
        .app
        .get_raw(&format!("/media/{}/2400", office.photo.as_str().unwrap()))
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a hero photo has its 2400 px variant"
    );

    // A new photo releases the old one to the sweep.
    let photo = office
        .app
        .upload("/home/hero/photo", common::png(64, 32), &office.publisher)
        .await
        .body["id"]
        .clone();
    let mut replaced = office.hero();
    replaced["photo"] = photo;
    let saved = office
        .call("PUT", "/home/hero", Some(&replaced), &office.publisher)
        .await;
    assert_eq!(saved.status, StatusCode::NO_CONTENT);
    let released: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT sweep_after FROM files WHERE id = $1::text::uuid")
            .bind(office.photo.as_str())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(released.is_some(), "the replaced photo is swept later");
}

#[sqlx::test]
async fn only_a_publisher_saves_the_hero_or_uploads_its_photo(pool: PgPool) {
    let office = office(&pool).await;

    let by_writer = office
        .call("PUT", "/home/hero", Some(&office.hero()), &office.writer)
        .await;
    let anonymous = office
        .app
        .call("PUT", "/home/hero", Some(&office.hero()), None)
        .await;
    let upload = office
        .app
        .upload("/home/hero/photo", common::png(8, 8), &office.writer)
        .await;

    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    assert_eq!(upload.status, StatusCode::FORBIDDEN);
    assert_eq!(office.app.get("/home/hero").await.0, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn a_malformed_hero_is_refused(pool: PgPool) {
    let office = office(&pool).await;
    let link = json!({ "label": "Berita", "path": "/berita" });
    let with = |field: &str, value: Value| {
        let mut hero = office.hero();
        hero[field] = value;
        hero
    };

    for hero in [
        with("photoAlt", json!(" ")),
        with("title", json!("")),
        with("photo", json!("00000000-0000-7000-8000-000000000000")),
        with("buttonPath", Value::Null),
        with("buttonPath", json!("https://elsewhere.example")),
        with("buttonPath", json!("//elsewhere.example")),
        with("quickLinks", json!([link, link, link, link, link])),
        with("quickLinks", json!([{ "label": "", "path": "/berita" }])),
        with(
            "quickLinks",
            json!([{ "label": "Berita", "path": "berita" }]),
        ),
    ] {
        let reply = office
            .call("PUT", "/home/hero", Some(&hero), &office.publisher)
            .await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{hero}");
    }

    // A hero without a button and with four Quick Links is fine.
    let mut hero = with("quickLinks", json!([link, link, link, link]));
    hero["buttonLabel"] = Value::Null;
    hero["buttonPath"] = json!(" ");
    let reply = office
        .call("PUT", "/home/hero", Some(&hero), &office.publisher)
        .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
}
