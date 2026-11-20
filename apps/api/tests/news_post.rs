mod common;

use axum::http::StatusCode;
use common::{Reply, TestApp};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A Village with a writer (`content.write`) and a publisher (`content.publish`, `audit.read`),
/// a Category and an uploaded cover.
struct Newsroom {
    app: TestApp,
    writer: String,
    publisher: String,
    category: Value,
    cover: Value,
}

async fn newsroom(pool: &PgPool) -> Newsroom {
    pranata::village::set_code(pool, "1101012001")
        .await
        .unwrap();
    let app = TestApp::new(pool.clone());
    let writer = app.user_with(pool, "penulis", &["content.write"]).await;
    let publisher = app
        .user_with(pool, "redaksi", &["content.publish", "audit.read"])
        .await;
    let category = app
        .call(
            "POST",
            "/categories",
            Some(&json!({ "name": "Pembangunan" })),
            Some(&publisher),
        )
        .await
        .body["id"]
        .clone();
    let cover = app
        .upload("/news-posts/cover", common::png(64, 48), &writer)
        .await
        .body["id"]
        .clone();
    Newsroom {
        app,
        writer,
        publisher,
        category,
        cover,
    }
}

fn body() -> Value {
    json!({ "type": "doc", "content": [
        { "type": "paragraph", "content": [
            { "type": "text", "text": "Jalan desa " },
            { "type": "text", "text": "sudah diaspal", "marks": [{ "type": "bold" }] },
        ] },
        { "type": "youtube", "attrs": { "videoId": "dQw4w9WgXcQ" } },
    ] })
}

impl Newsroom {
    /// Every field publishing requires.
    fn complete(&self, title: &str) -> Value {
        json!({
            "title": title, "category": self.category,
            "summary": " Pengaspalan jalan dusun selesai tepat waktu. ",
            "cover": self.cover, "coverCaption": "Jalan baru", "coverAlt": "Jalan beraspal",
            "body": body(),
        })
    }

    async fn call(&self, method: &str, path: &str, body: Option<&Value>, cookie: &str) -> Reply {
        self.app.call(method, path, body, Some(cookie)).await
    }

    /// A new Draft's id and slug.
    async fn draft(&self, fields: &Value) -> (String, String) {
        let reply = self
            .call("POST", "/news-posts", Some(fields), &self.writer)
            .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{fields}");
        let id = reply.body["id"].as_str().unwrap().to_owned();
        let post = self
            .call("GET", &format!("/news-posts/{id}"), None, &self.writer)
            .await;
        (id, post.body["slug"].as_str().unwrap().to_owned())
    }

    async fn published(&self, title: &str) -> (String, String) {
        let (id, slug) = self.draft(&self.complete(title)).await;
        let reply = self
            .call(
                "POST",
                &format!("/news-posts/{id}/publish"),
                None,
                &self.publisher,
            )
            .await;
        assert_eq!(reply.status, StatusCode::NO_CONTENT);
        (id, slug)
    }
}

#[sqlx::test]
async fn a_writer_drafts_a_publisher_publishes_and_the_public_reads_it(pool: PgPool) {
    let room = newsroom(&pool).await;
    let (id, slug) = room.draft(&json!({ "title": "Jalan Dusun Diaspal" })).await;
    assert_eq!(slug, "jalan-dusun-diaspal");
    assert_eq!(
        room.app
            .get(&format!("/news-posts/published/{slug}"))
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    // The autosave fills in the rest; the writer cannot publish.
    let path = format!("/news-posts/{id}");
    let saved = room
        .call(
            "PUT",
            &path,
            Some(&room.complete("Jalan Dusun Diaspal")),
            &room.writer,
        )
        .await;
    assert_eq!(saved.status, StatusCode::NO_CONTENT);
    let refused = room
        .call("POST", &format!("{path}/publish"), None, &room.writer)
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    let published = room
        .call("POST", &format!("{path}/publish"), None, &room.publisher)
        .await;
    assert_eq!(published.status, StatusCode::NO_CONTENT);

    let (status, post) = room.app.get(&format!("/news-posts/published/{slug}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(post["title"], "Jalan Dusun Diaspal");
    assert_eq!(
        post["summary"],
        "Pengaspalan jalan dusun selesai tepat waktu."
    );
    assert_eq!(
        post["category"],
        json!({ "id": room.category, "name": "Pembangunan" })
    );
    assert_eq!(post["cover"], room.cover);
    assert_eq!(post["coverCaption"], "Jalan baru");
    assert_eq!(post["coverAlt"], "Jalan beraspal");
    assert_eq!(post["body"], body());
    assert_eq!(post["readingMinutes"], 1);
    assert_eq!(
        post["author"],
        json!({ "name": "penulis", "position": null })
    );
    assert!(post["publishedAt"].is_string());
    let (_, page) = room.app.get("/news-posts/published").await;
    assert_eq!(page["total"], 1);
    assert_eq!(page["posts"][0]["slug"], slug);
    assert_eq!(page["posts"][0]["category"], "Pembangunan");

    // A live post changes only on a publisher's explicit save.
    let mut edit = room.complete("Jalan Dusun Sudah Diaspal");
    edit["slug"] = json!(slug);
    let by_writer = room.call("PUT", &path, Some(&edit), &room.writer).await;
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    let by_publisher = room.call("PUT", &path, Some(&edit), &room.publisher).await;
    assert_eq!(by_publisher.status, StatusCode::NO_CONTENT);
    let (_, post) = room.app.get(&format!("/news-posts/published/{slug}")).await;
    assert_eq!(post["title"], "Jalan Dusun Sudah Diaspal");

    let entries = room
        .call("GET", "/audit-entries", None, &room.publisher)
        .await
        .body["entries"]
        .clone();
    let actions: Vec<_> = entries
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["action"].as_str().unwrap())
        .filter(|action| action.starts_with("news_post."))
        .collect();
    assert_eq!(
        actions,
        [
            "news_post.update",
            "news_post.publish",
            "news_post.update",
            "news_post.create"
        ]
    );
    assert_eq!(
        entries[0]["changes"],
        json!([{
            "field": "title", "from": "Jalan Dusun Diaspal", "to": "Jalan Dusun Sudah Diaspal",
            "sensitive": false,
        }])
    );
}

#[sqlx::test]
async fn publishing_needs_a_category_summary_cover_and_alt_text(pool: PgPool) {
    let room = newsroom(&pool).await;
    for missing in ["category", "summary", "cover", "coverAlt"] {
        let mut fields = room.complete("Rapat");
        fields[missing] = Value::Null;
        let (id, _) = room.draft(&fields).await;

        let reply = room
            .call(
                "POST",
                &format!("/news-posts/{id}/publish"),
                None,
                &room.publisher,
            )
            .await;

        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{missing}");
    }

    // Nor can a live post lose one.
    let (id, _) = room.published("Rapat").await;
    let mut fields = room.complete("Rapat");
    fields["summary"] = json!("");
    let reply = room
        .call(
            "PUT",
            &format!("/news-posts/{id}"),
            Some(&fields),
            &room.publisher,
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn slugs_are_transliterated_unique_and_locked_at_first_publish(pool: PgPool) {
    let room = newsroom(&pool).await;
    let (first, slug) = room
        .draft(&json!({ "title": "Musyawarah Désa 2027" }))
        .await;
    let (_, second) = room
        .draft(&json!({ "title": "Musyawarah desa: 2027!" }))
        .await;
    let (_, custom) = room
        .draft(&json!({ "title": "Lain", "slug": "Musyawarah Desa 2027" }))
        .await;
    assert_eq!(slug, "musyawarah-desa-2027");
    assert_eq!(second, "musyawarah-desa-2027-2");
    assert_eq!(custom, "musyawarah-desa-2027-3");

    // Editable while a Draft, blank to follow the title.
    let path = format!("/news-posts/{first}");
    let mut fields = room.complete("Musyawarah Desa 2027");
    fields["slug"] = json!("musdes");
    room.call("PUT", &path, Some(&fields), &room.writer).await;
    fields["slug"] = json!("");
    room.call("PUT", &path, Some(&fields), &room.writer).await;
    let post = room.call("GET", &path, None, &room.writer).await;
    assert_eq!(post.body["slug"], "musyawarah-desa-2027");

    // Locked once published, even after an unpublish.
    room.call("POST", &format!("{path}/publish"), None, &room.publisher)
        .await;
    room.call("POST", &format!("{path}/unpublish"), None, &room.publisher)
        .await;
    fields["slug"] = json!("musdes");
    let refused = room.call("PUT", &path, Some(&fields), &room.writer).await;
    assert_eq!(refused.status, StatusCode::CONFLICT);
    fields["slug"] = json!("musyawarah-desa-2027");
    fields["title"] = json!("Judul Baru");
    let kept = room.call("PUT", &path, Some(&fields), &room.writer).await;
    assert_eq!(kept.status, StatusCode::NO_CONTENT);
    let post = room.call("GET", &path, None, &room.writer).await;
    assert_eq!(post.body["slug"], "musyawarah-desa-2027");

    // A deleted Draft's slug is free again.
    let (id, _) = room.draft(&json!({ "title": "Kerja Bakti" })).await;
    room.call(
        "DELETE",
        &format!("/news-posts/{id}"),
        None,
        &room.publisher,
    )
    .await;
    let (_, again) = room.draft(&json!({ "title": "Kerja Bakti" })).await;
    assert_eq!(again, "kerja-bakti");
}

#[sqlx::test]
async fn unpublishing_hides_a_post_and_keeps_its_first_published_date(pool: PgPool) {
    let room = newsroom(&pool).await;
    let (id, slug) = room.published("Posyandu Balita").await;
    let public = format!("/news-posts/published/{slug}");
    let date = room.app.get(&public).await.1["publishedAt"].clone();

    let reply = room
        .call(
            "POST",
            &format!("/news-posts/{id}/unpublish"),
            None,
            &room.publisher,
        )
        .await;

    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    assert_eq!(room.app.get(&public).await.0, StatusCode::NOT_FOUND);
    assert_eq!(room.app.get("/news-posts/published").await.1["total"], 0);
    let again = room
        .call(
            "POST",
            &format!("/news-posts/{id}/unpublish"),
            None,
            &room.publisher,
        )
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    room.call(
        "POST",
        &format!("/news-posts/{id}/publish"),
        None,
        &room.publisher,
    )
    .await;
    assert_eq!(room.app.get(&public).await.1["publishedAt"], date);
}

#[sqlx::test]
async fn only_a_publisher_deletes_and_only_a_draft(pool: PgPool) {
    let room = newsroom(&pool).await;
    let (id, slug) = room.published("Bantuan Bibit").await;
    let path = format!("/news-posts/{id}");

    let live = room.call("DELETE", &path, None, &room.publisher).await;
    room.call("POST", &format!("{path}/unpublish"), None, &room.publisher)
        .await;
    let by_writer = room.call("DELETE", &path, None, &room.writer).await;
    let deleted = room.call("DELETE", &path, None, &room.publisher).await;

    assert_eq!(live.status, StatusCode::CONFLICT);
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    assert_eq!(
        room.call("GET", &path, None, &room.writer).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        room.app
            .get(&format!("/news-posts/published/{slug}"))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test]
async fn a_body_outside_the_allowlist_is_refused(pool: PgPool) {
    let room = newsroom(&pool).await;
    for content in [
        json!([{ "type": "image", "attrs": { "src": "https://x.example/a.png" } }]),
        json!([{ "type": "paragraph", "content": [{ "type": "text", "text": "x", "marks": [
            { "type": "link", "attrs": { "href": "javascript:alert(1)" } },
        ] }] }]),
    ] {
        let fields = json!({ "title": "Uji", "body": { "type": "doc", "content": content } });

        let reply = room
            .call("POST", "/news-posts", Some(&fields), &room.writer)
            .await;

        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{content}");
    }
}

#[sqlx::test]
async fn people_without_a_content_permission_see_no_drafts(pool: PgPool) {
    let room = newsroom(&pool).await;
    room.draft(&json!({ "title": "Rahasia" })).await;
    let manager = room
        .app
        .user_with(&pool, "operator", &["village.manage"])
        .await;

    assert_eq!(
        room.call("GET", "/news-posts", None, &manager).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        room.app.request("GET", "/news-posts").await,
        StatusCode::UNAUTHORIZED
    );
    let drafts = room.call("GET", "/news-posts", None, &room.writer).await;
    assert_eq!(drafts.body[0]["title"], "Rahasia");
    assert_eq!(drafts.body[0]["published"], false);
}

#[sqlx::test]
async fn a_category_in_use_cannot_be_deleted(pool: PgPool) {
    let room = newsroom(&pool).await;
    room.draft(&room.complete("Rapat")).await;

    let reply = room
        .call(
            "DELETE",
            &format!("/categories/{}", room.category.as_str().unwrap()),
            None,
            &room.publisher,
        )
        .await;

    assert_eq!(reply.status, StatusCode::CONFLICT);
}

#[sqlx::test]
async fn the_public_list_is_newest_first_by_category_and_a_page_at_a_time(pool: PgPool) {
    let room = newsroom(&pool).await;
    for n in 1..=10 {
        room.published(&format!("Berita {n}")).await;
    }
    let health = room
        .call(
            "POST",
            "/categories",
            Some(&json!({ "name": "Kesehatan" })),
            &room.publisher,
        )
        .await
        .body["id"]
        .clone();
    let mut fields = room.complete("Posyandu");
    fields["category"] = health.clone();
    let (id, _) = room.draft(&fields).await;
    room.call(
        "POST",
        &format!("/news-posts/{id}/publish"),
        None,
        &room.publisher,
    )
    .await;
    room.draft(&room.complete("Masih Draf")).await;

    let (_, first) = room.app.get("/news-posts/published").await;
    let (_, second) = room.app.get("/news-posts/published?page=2").await;
    let (_, filtered) = room
        .app
        .get(&format!(
            "/news-posts/published?category={}",
            health.as_str().unwrap()
        ))
        .await;

    let titles = |page: &Value| -> Vec<String> {
        page["posts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|post| post["title"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(first["total"], 11);
    assert_eq!(first["pageSize"], 9);
    assert_eq!(titles(&first)[..3], ["Posyandu", "Berita 10", "Berita 9"]);
    assert_eq!(titles(&second), ["Berita 2", "Berita 1"]);
    assert_eq!(filtered["total"], 1);
    assert_eq!(titles(&filtered), ["Posyandu"]);
}

#[sqlx::test]
async fn the_author_is_the_linked_official_with_their_position_today(pool: PgPool) {
    let room = newsroom(&pool).await;
    sqlx::query(
        "WITH official AS (INSERT INTO officials (name) VALUES ('Siti Aminah') RETURNING id),
         tenure AS (INSERT INTO tenures (position_id, official_id, kind, start_date,
             decree_number, decree_date)
             SELECT positions.id, official.id, 'definitive', '2020-01-01', '1/2020', '2020-01-01'
             FROM positions, official WHERE positions.name = 'Sekretaris Desa')
         UPDATE users SET official_id = official.id FROM official WHERE username = 'penulis'",
    )
    .execute(&pool)
    .await
    .unwrap();

    let (_, slug) = room.published("Rapat Perangkat").await;

    let (_, post) = room.app.get(&format!("/news-posts/published/{slug}")).await;
    assert_eq!(
        post["author"],
        json!({ "name": "Siti Aminah", "position": "Sekretaris Desa" })
    );
}
