mod common;

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use common::{Reply, TestApp};
use serde_json::{Value, json};
use sqlx::PgPool;

/// 15:30 UTC on 20 November 2026: 22:30 on the 20th in WIB, 00:30 on the 21st in WIT.
fn evening() -> DateTime<Utc> {
    "2026-11-20T15:30:00Z".parse().unwrap()
}

/// A Village with `code` and a clock at `now`, a writer (`content.write`) and a publisher
/// (`content.publish`, `audit.read`).
struct Board {
    app: TestApp,
    writer: String,
    publisher: String,
}

async fn board(pool: &PgPool, code: &str, now: Option<DateTime<Utc>>) -> Board {
    pranata::village::set_code(pool, code).await.unwrap();
    let app = TestApp::at(pool.clone(), now);
    let writer = app.user_with(pool, "penulis", &["content.write"]).await;
    let publisher = app
        .user_with(pool, "redaksi", &["content.publish", "audit.read"])
        .await;
    Board {
        app,
        writer,
        publisher,
    }
}

fn body() -> Value {
    json!({ "type": "doc", "content": [
        { "type": "paragraph", "content": [{ "type": "text", "text": "Bawa KTP dan KK." }] },
    ] })
}

/// Every field publishing requires, valid from `start` to `end`.
fn complete(title: &str, start: &str, end: &str) -> Value {
    json!({
        "title": title, "summary": " Pelayanan KTP ditutup sementara. ",
        "startDate": start, "endDate": end,
    })
}

impl Board {
    async fn call(&self, method: &str, path: &str, body: Option<&Value>, cookie: &str) -> Reply {
        self.app.call(method, path, body, Some(cookie)).await
    }

    /// A new Draft's id and slug.
    async fn draft(&self, fields: &Value) -> (String, String) {
        let reply = self
            .call("POST", "/announcements", Some(fields), &self.writer)
            .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{fields}");
        let id = reply.body["id"].as_str().unwrap().to_owned();
        let announcement = self
            .call("GET", &format!("/announcements/{id}"), None, &self.writer)
            .await;
        (id, announcement.body["slug"].as_str().unwrap().to_owned())
    }

    async fn publish(&self, id: &str) -> Reply {
        self.call(
            "POST",
            &format!("/announcements/{id}/publish"),
            None,
            &self.publisher,
        )
        .await
    }

    async fn published(&self, fields: &Value) -> String {
        let (id, slug) = self.draft(fields).await;
        assert_eq!(self.publish(&id).await.status, StatusCode::NO_CONTENT);
        slug
    }

    /// The validity of each Announcement in the public list, by slug.
    async fn validities(&self) -> Vec<(String, String)> {
        let (status, page) = self.app.get("/announcements/published").await;
        assert_eq!(status, StatusCode::OK);
        page["announcements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                (
                    a["slug"].as_str().unwrap().to_owned(),
                    a["validity"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    }
}

#[sqlx::test]
async fn a_writer_drafts_a_publisher_publishes_and_the_public_reads_it(pool: PgPool) {
    let board = board(&pool, "1101012001", None).await;
    let (id, slug) = board
        .draft(&json!({ "title": "Libur Pelayanan KTP" }))
        .await;
    assert_eq!(slug, "libur-pelayanan-ktp");

    // Publishing needs the summary and both dates.
    assert_eq!(
        board.publish(&id).await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let path = format!("/announcements/{id}");
    let mut fields = complete("Libur Pelayanan KTP", "2020-01-01", "2099-12-31");
    fields["body"] = body();
    let saved = board.call("PUT", &path, Some(&fields), &board.writer).await;
    assert_eq!(saved.status, StatusCode::NO_CONTENT);
    let refused = board
        .call("POST", &format!("{path}/publish"), None, &board.writer)
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(board.publish(&id).await.status, StatusCode::NO_CONTENT);

    let (status, announcement) = board
        .app
        .get(&format!("/announcements/published/{slug}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(announcement["title"], "Libur Pelayanan KTP");
    assert_eq!(announcement["summary"], "Pelayanan KTP ditutup sementara.");
    assert_eq!(announcement["startDate"], "2020-01-01");
    assert_eq!(announcement["endDate"], "2099-12-31");
    assert_eq!(announcement["validity"], "inEffect");
    assert_eq!(announcement["body"], body());
    assert!(announcement["publishedAt"].is_string());
    let (_, page) = board.app.get("/announcements/published").await;
    assert_eq!(page["total"], 1);
    assert_eq!(page["announcements"][0]["hasBody"], true);
    let (_, in_effect) = board.app.get("/announcements/in-effect").await;
    assert_eq!(in_effect[0]["slug"], slug);

    // A live Announcement changes only on a publisher's explicit save.
    let by_writer = board.call("PUT", &path, Some(&fields), &board.writer).await;
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    fields["title"] = json!("Libur Pelayanan KTP Diperpanjang");
    let by_publisher = board
        .call("PUT", &path, Some(&fields), &board.publisher)
        .await;
    assert_eq!(by_publisher.status, StatusCode::NO_CONTENT);

    // Only a Draft is deleted, and its slug is free again.
    let live = board.call("DELETE", &path, None, &board.publisher).await;
    assert_eq!(live.status, StatusCode::CONFLICT);
    let unpublished = board
        .call("POST", &format!("{path}/unpublish"), None, &board.publisher)
        .await;
    assert_eq!(unpublished.status, StatusCode::NO_CONTENT);
    assert_eq!(
        board
            .app
            .get(&format!("/announcements/published/{slug}"))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let deleted = board.call("DELETE", &path, None, &board.publisher).await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);

    let entries = board
        .call("GET", "/audit-entries", None, &board.publisher)
        .await
        .body["entries"]
        .clone();
    let actions: Vec<_> = entries
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["action"].as_str().unwrap())
        .filter(|action| action.starts_with("announcement."))
        .collect();
    assert_eq!(
        actions,
        [
            "announcement.delete",
            "announcement.unpublish",
            "announcement.update",
            "announcement.publish",
            "announcement.update",
            "announcement.create"
        ]
    );
}

#[sqlx::test]
async fn an_announcement_without_a_body_has_no_detail_page(pool: PgPool) {
    let board = board(&pool, "1101012001", None).await;
    let mut fields = complete("Kerja Bakti", "2020-01-01", "2020-01-01");
    // The editor's empty document is one empty paragraph.
    fields["body"] = json!({ "type": "doc", "content": [{ "type": "paragraph" }] });
    let slug = board.published(&fields).await;

    let (_, page) = board.app.get("/announcements/published").await;
    assert_eq!(page["announcements"][0]["hasBody"], false);
    assert_eq!(
        board
            .app
            .get(&format!("/announcements/published/{slug}"))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test]
async fn dates_are_inclusive_and_ordered(pool: PgPool) {
    let board = board(&pool, "1101012001", None).await;
    let backwards = board
        .call(
            "POST",
            "/announcements",
            Some(&complete("Rapat", "2026-11-21", "2026-11-20")),
            &board.writer,
        )
        .await;
    assert_eq!(backwards.status, StatusCode::UNPROCESSABLE_ENTITY);
    let one_day = board
        .call(
            "POST",
            "/announcements",
            Some(&complete("Rapat", "2026-11-21", "2026-11-21")),
            &board.writer,
        )
        .await;
    assert_eq!(one_day.status, StatusCode::CREATED);
}

#[sqlx::test]
async fn validity_follows_the_village_day_in_wit(pool: PgPool) {
    // In WIT it is already the 21st.
    let board = board(&pool, "9103012004", Some(evening())).await;
    let starts_today = board
        .published(&complete("Mulai hari ini", "2026-11-21", "2026-11-21"))
        .await;
    let ends_today = board
        .published(&complete("Berakhir hari ini", "2026-11-01", "2026-11-21"))
        .await;
    let ended_yesterday = board
        .published(&complete("Berakhir kemarin", "2026-11-01", "2026-11-20"))
        .await;
    let starts_tomorrow = board
        .published(&complete("Mulai besok", "2026-11-22", "2026-11-30"))
        .await;

    let mut validities = board.validities().await;
    validities.sort();
    let mut expected = vec![
        (starts_today.clone(), "inEffect".to_owned()),
        (ends_today.clone(), "inEffect".to_owned()),
        (ended_yesterday, "ended".to_owned()),
        (starts_tomorrow, "upcoming".to_owned()),
    ];
    expected.sort();
    assert_eq!(validities, expected);

    let (_, in_effect) = board.app.get("/announcements/in-effect").await;
    let mut slugs: Vec<_> = in_effect
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["slug"].as_str().unwrap().to_owned())
        .collect();
    slugs.sort();
    let mut expected = vec![starts_today, ends_today];
    expected.sort();
    assert_eq!(slugs, expected);
}

#[sqlx::test]
async fn validity_follows_the_village_day_in_wib(pool: PgPool) {
    // In WIB it is still the 20th.
    let board = board(&pool, "1101012001", Some(evening())).await;
    let starts_on_the_21st = board
        .published(&complete("Mulai tanggal 21", "2026-11-21", "2026-11-21"))
        .await;
    let ends_on_the_20th = board
        .published(&complete("Berakhir tanggal 20", "2026-11-01", "2026-11-20"))
        .await;

    let mut validities = board.validities().await;
    validities.sort();
    let mut expected = vec![
        (starts_on_the_21st, "upcoming".to_owned()),
        (ends_on_the_20th.clone(), "inEffect".to_owned()),
    ];
    expected.sort();
    assert_eq!(validities, expected);
    let (_, in_effect) = board.app.get("/announcements/in-effect").await;
    assert_eq!(in_effect.as_array().unwrap().len(), 1);
    assert_eq!(in_effect[0]["slug"], ends_on_the_20th);
}
