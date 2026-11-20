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

/// A Village with `code` and a clock at `now`, a writer (`content.write`), a publisher
/// (`content.publish`, `audit.read`) and a Category.
struct Board {
    app: TestApp,
    writer: String,
    publisher: String,
    category: Value,
}

async fn board(pool: &PgPool, code: &str, now: Option<DateTime<Utc>>) -> Board {
    pranata::village::set_code(pool, code).await.unwrap();
    let app = TestApp::at(pool.clone(), now);
    let writer = app.user_with(pool, "penulis", &["content.write"]).await;
    let publisher = app
        .user_with(pool, "redaksi", &["content.publish", "audit.read"])
        .await;
    let category = app
        .call(
            "POST",
            "/categories",
            Some(&json!({ "name": "Kegiatan Warga" })),
            Some(&publisher),
        )
        .await
        .body["id"]
        .clone();
    Board {
        app,
        writer,
        publisher,
        category,
    }
}

impl Board {
    async fn call(&self, method: &str, path: &str, body: Option<&Value>, cookie: &str) -> Reply {
        self.app.call(method, path, body, Some(cookie)).await
    }

    /// Every field publishing requires, starting at `start` and ending at `end` if any.
    fn complete(&self, title: &str, start: &str, end: Option<&str>) -> Value {
        json!({
            "title": title, "category": self.category, "startsAt": start, "endsAt": end,
            "location": " Balai desa ",
        })
    }

    /// A new Draft's id.
    async fn draft(&self, fields: &Value) -> String {
        let reply = self
            .call("POST", "/events", Some(fields), &self.writer)
            .await;
        assert_eq!(reply.status, StatusCode::CREATED, "{fields}");
        reply.body["id"].as_str().unwrap().to_owned()
    }

    async fn publish(&self, id: &str) -> Reply {
        self.call(
            "POST",
            &format!("/events/{id}/publish"),
            None,
            &self.publisher,
        )
        .await
    }

    async fn published(&self, fields: &Value) -> String {
        let id = self.draft(fields).await;
        assert_eq!(self.publish(&id).await.status, StatusCode::NO_CONTENT);
        id
    }

    /// The titles in a public list, in its order.
    async fn titles(&self, path: &str) -> Vec<String> {
        let (status, list) = self.app.get(path).await;
        assert_eq!(status, StatusCode::OK);
        let events = list.get("events").unwrap_or(&list);
        events
            .as_array()
            .unwrap()
            .iter()
            .map(|event| event["title"].as_str().unwrap().to_owned())
            .collect()
    }
}

#[sqlx::test]
async fn a_writer_drafts_a_publisher_publishes_and_the_public_sees_it(pool: PgPool) {
    let board = board(&pool, "1101012001", None).await;
    let id = board.draft(&json!({ "title": "Kerja Bakti" })).await;

    // Publishing needs the Category, the start and the location.
    assert_eq!(
        board.publish(&id).await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let path = format!("/events/{id}");
    let mut fields = board.complete("Kerja Bakti", "2099-01-02T07:00:00", None);
    fields["description"] = json!(" Bawa cangkul. ");
    let saved = board.call("PUT", &path, Some(&fields), &board.writer).await;
    assert_eq!(saved.status, StatusCode::NO_CONTENT);
    let refused = board
        .call("POST", &format!("{path}/publish"), None, &board.writer)
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(board.publish(&id).await.status, StatusCode::NO_CONTENT);

    let (status, upcoming) = board.app.get("/events/upcoming").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        upcoming,
        json!([{
            "id": id, "title": "Kerja Bakti", "category": "Kegiatan Warga",
            "startsAt": "2099-01-02T07:00:00", "endsAt": null, "location": "Balai desa",
            "description": "Bawa cangkul.", "cancelled": false,
        }])
    );
    let list = board.call("GET", "/events", None, &board.writer).await.body;
    assert_eq!(list[0]["timing"], "upcoming");
    assert_eq!(list[0]["category"], "Kegiatan Warga");

    // A live Event changes only on a publisher's explicit save; called off, it stays listed.
    fields["cancelled"] = json!(true);
    let by_writer = board.call("PUT", &path, Some(&fields), &board.writer).await;
    assert_eq!(by_writer.status, StatusCode::FORBIDDEN);
    let by_publisher = board
        .call("PUT", &path, Some(&fields), &board.publisher)
        .await;
    assert_eq!(by_publisher.status, StatusCode::NO_CONTENT);
    let (_, upcoming) = board.app.get("/events/upcoming").await;
    assert_eq!(upcoming[0]["cancelled"], true);

    // Only a Draft is deleted.
    let live = board.call("DELETE", &path, None, &board.publisher).await;
    assert_eq!(live.status, StatusCode::CONFLICT);
    let unpublished = board
        .call("POST", &format!("{path}/unpublish"), None, &board.publisher)
        .await;
    assert_eq!(unpublished.status, StatusCode::NO_CONTENT);
    assert_eq!(board.titles("/events/upcoming").await, Vec::<String>::new());
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
        .filter(|action| action.starts_with("event."))
        .collect();
    assert_eq!(
        actions,
        [
            "event.delete",
            "event.unpublish",
            "event.update",
            "event.publish",
            "event.update",
            "event.create"
        ]
    );
}

#[sqlx::test]
async fn upcoming_and_past_follow_the_village_clock_in_wib(pool: PgPool) {
    // In WIB it is 22:30 on the 20th.
    let board = board(&pool, "1101012001", Some(evening())).await;
    for (title, start, end) in [
        (
            "Selesai sejam lalu",
            "2026-11-20T08:00:00",
            Some("2026-11-20T21:30:00"),
        ),
        (
            "Masih berlangsung",
            "2026-11-19T08:00:00",
            Some("2026-11-21T12:00:00"),
        ),
        ("Tadi pagi", "2026-11-20T07:00:00", None),
        ("Kemarin", "2026-11-19T19:00:00", None),
        ("Nanti malam", "2026-11-20T23:00:00", None),
        ("Besok", "2026-11-21T08:00:00", None),
    ] {
        board.published(&board.complete(title, start, end)).await;
    }

    // Upcoming soonest first; one under way still counts.
    assert_eq!(
        board.titles("/events/upcoming").await,
        ["Masih berlangsung", "Nanti malam", "Besok"]
    );
    // Past latest first; an Event without an end is past once it has started.
    assert_eq!(
        board.titles("/events/past").await,
        ["Selesai sejam lalu", "Tadi pagi", "Kemarin"]
    );
    let (_, page) = board.app.get("/events/past").await;
    assert_eq!(page["total"], 3);
}

#[sqlx::test]
async fn upcoming_and_past_follow_the_village_clock_in_wit(pool: PgPool) {
    // In WIT it is already 00:30 on the 21st, so 23:00 on the 20th has gone by.
    let board = board(&pool, "9103012004", Some(evening())).await;
    board
        .published(&board.complete("Nanti malam", "2026-11-20T23:00:00", None))
        .await;

    assert_eq!(board.titles("/events/upcoming").await, Vec::<String>::new());
    assert_eq!(board.titles("/events/past").await, ["Nanti malam"]);
    let list = board.call("GET", "/events", None, &board.writer).await.body;
    assert_eq!(list[0]["timing"], "past");
}

#[sqlx::test]
async fn past_events_come_ten_a_page(pool: PgPool) {
    let board = board(&pool, "1101012001", None).await;
    for day in 1..=11 {
        let start = format!("2020-01-{day:02}T08:00:00");
        board
            .published(&board.complete(&format!("Rapat {day}"), &start, None))
            .await;
    }

    let first = board.titles("/events/past").await;
    assert_eq!(first.len(), 10);
    assert_eq!(first[0], "Rapat 11");
    assert_eq!(board.titles("/events/past?page=2").await, ["Rapat 1"]);
    let (_, page) = board.app.get("/events/past?page=2").await;
    assert_eq!(page["total"], 11);
    assert_eq!(page["pageSize"], 10);
}

#[sqlx::test]
async fn an_end_may_span_days_but_never_precede_the_start(pool: PgPool) {
    let board = board(&pool, "1101012001", None).await;
    for (start, end, status) in [
        (
            "2026-11-21T08:00:00",
            "2026-11-21T07:59:00",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "2026-11-21T08:00:00",
            "2026-11-21T08:00:00",
            StatusCode::CREATED,
        ),
        (
            "2026-11-21T08:00:00",
            "2026-11-23T17:00:00",
            StatusCode::CREATED,
        ),
    ] {
        let reply = board
            .call(
                "POST",
                "/events",
                Some(&board.complete("Pameran", start, Some(end))),
                &board.writer,
            )
            .await;
        assert_eq!(reply.status, status, "{start} – {end}");
    }
}

#[sqlx::test]
async fn events_reuse_the_categories_of_news_posts(pool: PgPool) {
    let board = board(&pool, "1101012001", None).await;
    let unknown = board
        .call(
            "POST",
            "/events",
            Some(&json!({ "title": "Rapat", "category": "0190a000-0000-7000-8000-000000000000" })),
            &board.writer,
        )
        .await;
    assert_eq!(unknown.status, StatusCode::UNPROCESSABLE_ENTITY);

    // The Category the News Posts use files an Event, which then keeps it.
    board
        .draft(&board.complete("Rapat", "2026-11-21T08:00:00", None))
        .await;
    let reply = board
        .call(
            "DELETE",
            &format!("/categories/{}", board.category.as_str().unwrap()),
            None,
            &board.publisher,
        )
        .await;
    assert_eq!(reply.status, StatusCode::CONFLICT);
}
