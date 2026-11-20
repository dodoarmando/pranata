mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::{Value, json};
use sqlx::PgPool;

#[sqlx::test]
async fn lists_every_published_item_with_a_page_and_its_last_change(pool: PgPool) {
    pranata::user::create_administrator(&pool, "redaksi")
        .await
        .unwrap();
    let empty = json!({ "type": "doc", "content": [] });
    let text = json!({ "type": "doc", "content": [
        { "type": "paragraph", "content": [{ "type": "text", "text": "Isi" }] },
    ] });
    sqlx::query("INSERT INTO categories (name) VALUES ('Kegiatan')")
        .execute(&pool)
        .await
        .unwrap();
    let file: uuid::Uuid =
        sqlx::query_scalar("INSERT INTO files (byte_size) VALUES (2048) RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    for (slug, published) in [("jalan-diaspal", true), ("draf-berita", false)] {
        sqlx::query(
            "INSERT INTO news_posts (title, slug, category_id, summary, cover_file_id, cover_alt,
             body, reading_minutes, author_id, published, first_published_at, updated_at)
             VALUES ('Berita', $1, (SELECT id FROM categories), 'Kabar', $2, 'Foto', $3, 1,
             (SELECT id FROM users), $4, CASE WHEN $4 THEN now() END, '2026-11-01T03:00:00Z')",
        )
        .bind(slug)
        .bind(file)
        .bind(&text)
        .bind(published)
        .execute(&pool)
        .await
        .unwrap();
    }
    // Only an Announcement with a body has a page.
    for (slug, body) in [("kerja-bakti", &text), ("tanpa-isi", &empty)] {
        sqlx::query(
            "INSERT INTO announcements (title, slug, summary, body, start_date, end_date,
             author_id, published, first_published_at)
             VALUES ('Pengumuman', $1, 'Harap hadir', $2, '2026-01-01', '2026-01-31',
             (SELECT id FROM users), true, now())",
        )
        .bind(slug)
        .bind(body)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO pages (title, slug, menu, sort_order, summary, body, author_id, published,
         first_published_at) VALUES ('Sejarah', 'sejarah', 'profile', 1, 'Tentang', $1,
         (SELECT id FROM users), true, now())",
    )
    .bind(&text)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO albums (title, slug, occasion_start, author_id, published, first_published_at)
         VALUES ('HUT RI', 'hut-ri', '2026-08-17', (SELECT id FROM users), true, now())",
    )
    .execute(&pool)
    .await
    .unwrap();
    for (year, published) in [(2026, true), (2027, false)] {
        sqlx::query(
            "INSERT INTO apbdes_summaries (fiscal_year, version, version_date, budget, published)
             VALUES ($1, 'original', '2026-01-02', '{}', $2)",
        )
        .bind(year)
        .bind(published)
        .execute(&pool)
        .await
        .unwrap();
    }

    let (status, sitemap) = TestApp::new(pool).get("/sitemap").await;

    assert_eq!(status, StatusCode::OK);
    let field = |list: &str, key: &str| -> Vec<Value> {
        sitemap[list]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item[key].clone())
            .collect()
    };
    assert_eq!(field("newsPosts", "slug"), [json!("jalan-diaspal")]);
    assert_eq!(
        field("newsPosts", "updatedAt"),
        [json!("2026-11-01T03:00:00Z")]
    );
    assert_eq!(field("announcements", "slug"), [json!("kerja-bakti")]);
    assert_eq!(field("pages", "slug"), [json!("sejarah")]);
    assert_eq!(field("pages", "menu"), [json!("profile")]);
    assert_eq!(field("albums", "slug"), [json!("hut-ri")]);
    assert_eq!(field("apbdesSummaries", "fiscalYear"), [json!(2026)]);
    assert!(sitemap["albums"][0]["updatedAt"].is_string());
}
