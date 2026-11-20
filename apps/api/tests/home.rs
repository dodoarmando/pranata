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

/// An Official serving as head of village since 2024 and one whose Tenure as secretary ended
/// in 2025; their Tenure ids.
async fn officials(pool: &PgPool) -> (String, String) {
    let tenure = |name: &'static str, head: bool, end: Option<&'static str>| async move {
        sqlx::query_scalar::<_, String>(
            "WITH official AS (INSERT INTO officials (name) VALUES ($1) RETURNING id)
             INSERT INTO tenures (position_id, official_id, kind, start_date, end_date,
                 decree_number, decree_date)
             SELECT positions.id, official.id, 'definitive', '2024-01-01', $3::date, '1', '2024-01-01'
             FROM positions, official WHERE head_of_village = $2 AND name IS NOT NULL
             ORDER BY sort_order LIMIT 1 RETURNING id::text",
        )
        .bind(name)
        .bind(head)
        .bind(end)
        .fetch_one(pool)
        .await
        .unwrap()
    };
    (
        tenure("Budi Santoso", true, None).await,
        tenure("Siti Aminah", false, Some("2025-01-31")).await,
    )
}

#[sqlx::test]
async fn a_publisher_saves_the_welcome_live_signed_by_a_serving_official(pool: PgPool) {
    let office = office(&pool).await;
    let (head, former) = officials(&pool).await;
    let portrait = office
        .app
        .upload(
            "/home/welcome/photo",
            common::png(30, 40),
            &office.publisher,
        )
        .await;
    assert_eq!(portrait.status, StatusCode::CREATED);
    let welcome = |signer: &str| {
        json!({
            "photo": portrait.body["id"], "photoAlt": " Kepala desa ",
            "message": " Selamat datang di situs desa kami. ", "signer": signer,
        })
    };
    assert_eq!(
        office.app.get("/home/welcome").await.0,
        StatusCode::NOT_FOUND
    );

    let by_writer = office
        .call(
            "PUT",
            "/home/welcome",
            Some(&welcome(&head)),
            &office.writer,
        )
        .await;
    let ended = office
        .call(
            "PUT",
            "/home/welcome",
            Some(&welcome(&former)),
            &office.publisher,
        )
        .await;
    let saved = office
        .call(
            "PUT",
            "/home/welcome",
            Some(&welcome(&head)),
            &office.publisher,
        )
        .await;

    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    assert_eq!(ended.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(saved.status, StatusCode::NO_CONTENT);
    assert_eq!(
        office.app.get("/home/welcome").await,
        (
            StatusCode::OK,
            json!({
                "photo": portrait.body["id"], "photoAlt": "Kepala desa",
                "message": "Selamat datang di situs desa kami.", "signer": head,
                "signerName": "Budi Santoso", "signerPosition": "Kepala Desa",
                "signerKind": "definitive", "signerServing": true,
            })
        )
    );

    // Once the signer's Tenure ends the welcome says so, and still saves with them.
    sqlx::query("UPDATE tenures SET end_date = '2025-06-30' WHERE id = $1::uuid")
        .bind(&head)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        office.app.get("/home/welcome").await.1["signerServing"],
        false
    );
    let mut edited = welcome(&head);
    edited["message"] = json!("Salam sejahtera.");
    let saved = office
        .call("PUT", "/home/welcome", Some(&edited), &office.publisher)
        .await;
    assert_eq!(saved.status, StatusCode::NO_CONTENT);
}

#[sqlx::test]
async fn a_malformed_welcome_is_refused(pool: PgPool) {
    let office = office(&pool).await;
    let (head, _) = officials(&pool).await;
    let welcome = json!({
        "photo": office.photo, "photoAlt": "Kepala desa", "message": "Selamat datang.",
        "signer": head,
    });
    let with = |field: &str, value: Value| {
        let mut welcome = welcome.clone();
        welcome[field] = value;
        welcome
    };

    for welcome in [
        with("photoAlt", json!(" ")),
        with("message", json!("")),
        with("message", json!("a".repeat(2001))),
        with("photo", json!("00000000-0000-7000-8000-000000000000")),
        with("signer", json!("00000000-0000-7000-8000-000000000000")),
    ] {
        let reply = office
            .call("PUT", "/home/welcome", Some(&welcome), &office.publisher)
            .await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{welcome}");
    }
    let anonymous = office
        .app
        .call("PUT", "/home/welcome", Some(&welcome), None)
        .await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
}
