mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

/// A Tiptap body of one paragraph.
fn body(text: &str) -> Value {
    json!({ "type": "doc", "content": [
        { "type": "paragraph", "content": [{ "type": "text", "text": text }] },
    ] })
}

/// Content written straight to the database, so each test states only what search sees.
struct Site {
    app: TestApp,
    pool: PgPool,
    author: Uuid,
    file: Uuid,
}

async fn site(pool: &PgPool) -> Site {
    pranata::village::set_code(pool, "1101012001")
        .await
        .unwrap();
    pranata::user::create_administrator(pool, "redaksi")
        .await
        .unwrap();
    let author = sqlx::query_scalar("SELECT id FROM users")
        .fetch_one(pool)
        .await
        .unwrap();
    let file = sqlx::query_scalar("INSERT INTO files (byte_size) VALUES (2048) RETURNING id")
        .fetch_one(pool)
        .await
        .unwrap();
    Site {
        app: TestApp::new(pool.clone()),
        pool: pool.clone(),
        author,
        file,
    }
}

impl Site {
    async fn news_post(&self, slug: &str, title: &str, text: &str, published: bool) {
        sqlx::query(
            "INSERT INTO news_posts (title, slug, category_id, summary, cover_file_id, cover_alt,
             body, reading_minutes, author_id, published, first_published_at)
             VALUES ($1, $2, (SELECT id FROM categories), 'Kabar desa', $3, 'Foto', $4, 1, $5,
             $6, CASE WHEN $6 THEN now() END)",
        )
        .bind(title)
        .bind(slug)
        .bind(self.file)
        .bind(body(text))
        .bind(self.author)
        .bind(published)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn announcement(&self, slug: &str, title: &str, end_date: &str, published: bool) {
        sqlx::query(
            "INSERT INTO announcements (title, slug, summary, body, start_date, end_date,
             author_id, published, first_published_at)
             VALUES ($1, $2, 'Harap diperhatikan', $3, '2026-01-01', $4::date, $5, $6,
             CASE WHEN $6 THEN now() END)",
        )
        .bind(title)
        .bind(slug)
        .bind(json!({ "type": "doc", "content": [] }))
        .bind(end_date)
        .bind(self.author)
        .bind(published)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn page(&self, slug: &str, title: &str, text: &str, published: bool) {
        sqlx::query(
            "INSERT INTO pages (title, slug, menu, sort_order, summary, body, author_id,
             published, first_published_at)
             VALUES ($1, $2, 'profile', 1, 'Tentang desa', $3, $4, $5, CASE WHEN $5 THEN now() END)",
        )
        .bind(title)
        .bind(slug)
        .bind(body(text))
        .bind(self.author)
        .bind(published)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn document(&self, title: &str, published: bool) {
        sqlx::query(
            "INSERT INTO transparency_documents (title, document_type, fiscal_year, file_id,
             published) VALUES ($1, 'village_regulation', 2026, $2, $3)",
        )
        .bind(title)
        .bind(self.file)
        .bind(published)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn search(&self, query: &str) -> Value {
        let (status, page) = self.app.get(&format!("/search?{query}")).await;
        assert_eq!(status, StatusCode::OK, "{query}");
        page
    }
}

fn titles(page: &Value) -> Vec<&str> {
    page["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|result| result["title"].as_str().unwrap())
        .collect()
}

#[sqlx::test]
async fn one_word_form_finds_every_published_kind_and_never_a_draft(pool: PgPool) {
    let site = site(&pool).await;
    sqlx::query("INSERT INTO categories (name) VALUES ('Kegiatan')")
        .execute(&pool)
        .await
        .unwrap();
    site.news_post(
        "jalan",
        "Jalan Dusun Diaspal",
        "Pembangunan jalan selesai.",
        true,
    )
    .await;
    site.news_post("judul", "Pembangunan Balai Warga", "Gotong royong.", true)
        .await;
    site.news_post("draf", "Pembangunan Rahasia", "Belum terbit.", false)
        .await;
    // Ended long ago, still found.
    site.announcement("rapat", "Rapat Pembangunan Desa", "2026-01-02", true)
        .await;
    site.announcement("draf", "Pembangunan Draf", "2027-01-02", false)
        .await;
    site.page("sejarah", "Sejarah", "Desa dibangun pada 1901.", true)
        .await;
    site.page("draf", "Pembangunan Draf", "Belum terbit.", false)
        .await;
    site.document("Peraturan Pembangunan Desa", true).await;
    site.document("Peraturan Pembangunan Draf", false).await;

    let page = site.search("q=membangun").await;
    let mut found = titles(&page);
    // Title matches outrank body matches.
    let mut body_only = found[3..].to_vec();
    body_only.sort_unstable();
    assert_eq!(body_only, ["Jalan Dusun Diaspal", "Sejarah"], "{page}");
    found.sort_unstable();
    assert_eq!(
        found,
        [
            "Jalan Dusun Diaspal",
            "Pembangunan Balai Warga",
            "Peraturan Pembangunan Desa",
            "Rapat Pembangunan Desa",
            "Sejarah",
        ]
    );
    assert_eq!(page["total"], 5);
    assert_eq!(page["pageSize"], 10);

    let announcement = &page["results"].as_array().unwrap()[titles(&page)
        .iter()
        .position(|t| *t == "Rapat Pembangunan Desa")
        .unwrap()];
    assert_eq!(announcement["kind"], "announcement");
    assert_eq!(announcement["summary"], "Harap diperhatikan");
    // No body, so no page of its own.
    assert_eq!(announcement["slug"], Value::Null);
    assert!(announcement["datedAt"].is_string());

    let documents = site.search("q=perdes&type=transparencyDocument").await;
    assert_eq!(titles(&documents), ["Peraturan Pembangunan Desa"]);
    assert_eq!(documents["results"][0]["documentType"], "villageRegulation");
    assert_eq!(documents["results"][0]["fiscalYear"], 2026);
    assert_eq!(documents["results"][0]["file"], site.file.to_string());

    // The type label and year are searchable too.
    assert_eq!(
        titles(&site.search("q=perdes+2026").await),
        ["Peraturan Pembangunan Desa"]
    );
    let pages = site.search("q=sejarah&type=page").await;
    assert_eq!(pages["results"][0]["menu"], "profile");
    assert_eq!(pages["results"][0]["slug"], "sejarah");
}

#[sqlx::test]
async fn the_last_word_matches_as_a_prefix_and_pages_hold_ten(pool: PgPool) {
    let site = site(&pool).await;
    for n in 1..=11 {
        site.document(&format!("Laporan Keuangan {n}"), true).await;
    }
    let first = site.search("q=laporan+keua").await;
    assert_eq!(first["total"], 11);
    assert_eq!(titles(&first).len(), 10);
    // The newest first among equal ranks.
    assert_eq!(titles(&first)[0], "Laporan Keuangan 11");
    let second = site.search("q=laporan+keua&page=2").await;
    assert_eq!(titles(&second), ["Laporan Keuangan 1"]);
    assert_eq!(site.search("q=keuangan+-laporan").await["total"], 0);
}

#[sqlx::test]
async fn a_query_is_two_to_one_hundred_characters(pool: PgPool) {
    let site = site(&pool).await;
    let get = |query: String| {
        let app = &site.app;
        async move { app.get(&format!("/search?q={query}")).await.0 }
    };
    assert_eq!(get("a".into()).await, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(get("+a+".into()).await, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(get("ab".into()).await, StatusCode::OK);
    assert_eq!(get("%C3%A9".repeat(100)).await, StatusCode::OK);
    assert_eq!(get("a".repeat(101)).await, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(site.app.get("/search").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(
        site.app.get("/search?q=ab&type=letter").await.0,
        StatusCode::BAD_REQUEST
    );
}
