mod common;

use std::sync::atomic::{AtomicU32, Ordering};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use pranata::{
    auth,
    ids::{RoleId, UserId},
    region_codes::VillageCode,
    village,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};
use tower::ServiceExt;

const PASSWORD: &str = "correct horse battery";

struct Harness {
    pool: PgPool,
    app: Router,
    next_ip: AtomicU32,
}

struct Reply {
    status: StatusCode,
    headers: header::HeaderMap,
    text: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.text).unwrap_or(Value::Null)
    }
}

struct Session {
    cookie: String,
}

impl Harness {
    async fn new(pool: PgPool) -> Self {
        // Riau lies in WIB (+07:00).
        village::init(&pool, &VillageCode::parse("14.05.03.2002").unwrap())
            .await
            .unwrap();
        let app = pranata::app(common::test_state(pool.clone()));
        Self {
            pool,
            app,
            next_ip: AtomicU32::new(0),
        }
    }

    async fn role(&self, name: &str, permissions: &[&str]) -> RoleId {
        let id = sqlx::query_scalar!(
            r#"INSERT INTO roles (name) VALUES ($1) RETURNING id AS "id: RoleId""#,
            name
        )
        .fetch_one(&self.pool)
        .await
        .unwrap();
        for permission in permissions {
            sqlx::query!(
                "INSERT INTO role_permissions (role_id, permission) VALUES ($1, $2)",
                id.0,
                permission
            )
            .execute(&self.pool)
            .await
            .unwrap();
        }
        id
    }

    async fn user(&self, username: &str, display_name: &str, role: RoleId) -> UserId {
        let hash = auth::password::hash_password(PASSWORD).unwrap();
        sqlx::query_scalar!(
            r#"INSERT INTO users (username, display_name, password_hash, role_id)
               VALUES ($1, $2, $3, $4) RETURNING id AS "id: UserId""#,
            username,
            display_name,
            hash,
            role.0
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// A signed-in User holding `audit.read`, with an empty log: the village
    /// setup and the sign-in wrote entries a test does not want to count.
    async fn reader(&self) -> Session {
        let role = self.role("Auditor", &["audit.read"]).await;
        self.user("auditor", "Auditor", role).await;
        let session = self.login("auditor").await;
        sqlx::query!("DELETE FROM audit_entries")
            .execute(&self.pool)
            .await
            .unwrap();
        session
    }

    async fn login(&self, username: &str) -> Session {
        let n = self.next_ip.fetch_add(1, Ordering::Relaxed);
        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/auth/login")
            .header("x-forwarded-for", format!("10.2.0.{n}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"username": username, "password": PASSWORD}).to_string(),
            ))
            .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let cookie = response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        Session { cookie }
    }

    async fn get(&self, uri: &str, session: &Session) -> Reply {
        let request = Request::builder()
            .uri(uri)
            .header(header::COOKIE, &session.cookie)
            .body(Body::empty())
            .unwrap();
        let (parts, body) = self
            .app
            .clone()
            .oneshot(request)
            .await
            .unwrap()
            .into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        Reply {
            status: parts.status,
            headers: parts.headers,
            text: String::from_utf8(bytes.to_vec()).unwrap(),
        }
    }

    async fn entry(&self, at: OffsetDateTime, actor: Option<UserId>, action: &str, summary: &str) {
        sqlx::query!(
            "INSERT INTO audit_entries (at, actor, action, object_type, object_id, summary, changes)
             VALUES ($1, $2, $3, 'user', 'x', $4, $5)",
            at,
            actor.map(|user| user.0),
            action,
            summary,
            json!([{"field": "display_name", "before": "A", "after": "B"}]),
        )
        .execute(&self.pool)
        .await
        .unwrap();
    }
}

fn base() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_795_593_600).unwrap()
}

fn ids(reply: &Reply) -> Vec<String> {
    reply.json()["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap().to_owned())
        .collect()
}

#[sqlx::test]
async fn lists_newest_first_with_keyset_pages(pool: PgPool) {
    let h = Harness::new(pool).await;
    let session = h.reader().await;
    for minute in 0..120 {
        h.entry(
            base() + Duration::minutes(minute),
            None,
            "update",
            &format!("entry {minute}"),
        )
        .await;
    }

    let mut seen: Vec<String> = Vec::new();
    let mut sizes = Vec::new();
    let mut uri = "/api/admin/audit?limit=50".to_owned();
    loop {
        let page = h.get(&uri, &session).await;
        assert_eq!(page.status, StatusCode::OK, "{}", page.text);
        let page_ids = ids(&page);
        sizes.push(page_ids.len());
        seen.extend(page_ids);
        match page.json()["next"].as_str() {
            Some(next) => uri = format!("/api/admin/audit?limit=50&before={next}"),
            None => break,
        }
    }
    assert_eq!(sizes, [50, 50, 20]);
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 120, "no entry appears twice");

    let first = h.get("/api/admin/audit?limit=3", &session).await.json();
    let summaries: Vec<&str> = first["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["summary"].as_str().unwrap())
        .collect();
    assert_eq!(summaries, ["entry 119", "entry 118", "entry 117"]);
    assert_eq!(
        first["entries"][0]["changes"],
        json!([{"field": "display_name", "before": "A", "after": "B"}])
    );
    assert!(first["entries"][0]["actor"].is_null());
}

#[sqlx::test]
async fn malformed_cursor_is_422(pool: PgPool) {
    let h = Harness::new(pool).await;
    let session = h.reader().await;
    let reply = h
        .get("/api/admin/audit?before=not-a-cursor", &session)
        .await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        reply.json()["errors"],
        json!([{"field": "before", "code": "invalid"}])
    );
}

#[sqlx::test]
async fn filters_by_actor_action_and_village_dates(pool: PgPool) {
    let h = Harness::new(pool).await;
    let session = h.reader().await;
    let role = h.role("Staf", &[]).await;
    let neng = h.user("neng", "Neng", role).await;
    let at = OffsetDateTime::parse(
        "2026-11-17T17:30:00Z",
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    h.entry(at, Some(neng), "deactivate", "late").await;
    h.entry(at - Duration::days(3), None, "create", "early")
        .await;

    let summaries = |reply: &Reply| -> Vec<String> {
        reply.json()["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["summary"].as_str().unwrap().to_owned())
            .collect()
    };

    // 17:30Z is 00:30 on the 18th in WIB.
    let on_18th = h
        .get("/api/admin/audit?from=2026-11-18&to=2026-11-18", &session)
        .await;
    assert_eq!(summaries(&on_18th), ["late"]);
    let on_17th = h
        .get("/api/admin/audit?from=2026-11-17&to=2026-11-17", &session)
        .await;
    assert_eq!(summaries(&on_17th), Vec::<String>::new());

    let by_actor = h
        .get(&format!("/api/admin/audit?actor={}", neng.0), &session)
        .await;
    assert_eq!(summaries(&by_actor), ["late"]);
    assert_eq!(by_actor.json()["entries"][0]["actor"]["username"], "neng");
    assert_eq!(
        by_actor.json()["entries"][0]["actor"]["display_name"],
        "Neng"
    );

    let by_action = h.get("/api/admin/audit?action=create", &session).await;
    assert_eq!(summaries(&by_action), ["early"]);
    let by_type = h.get("/api/admin/audit?object_type=role", &session).await;
    assert_eq!(summaries(&by_type), Vec::<String>::new());
}

#[sqlx::test]
async fn csv_has_header_and_rows(pool: PgPool) {
    let h = Harness::new(pool).await;
    let session = h.reader().await;
    let at = OffsetDateTime::parse(
        "2026-11-17T17:30:00Z",
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    h.entry(at, None, "update", "first").await;
    h.entry(at + Duration::minutes(1), None, "create", "second")
        .await;

    let reply = h.get("/api/admin/audit.csv", &session).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(
        reply.headers[header::CONTENT_TYPE],
        "text/csv; charset=utf-8"
    );
    assert_eq!(
        reply.headers[header::CONTENT_DISPOSITION],
        "attachment; filename=\"log-audit.csv\""
    );
    let lines: Vec<&str> = reply.text.lines().collect();
    assert_eq!(lines.len(), 3, "{}", reply.text);
    assert_eq!(
        lines[0],
        "waktu,pengguna,tindakan,objek,id_objek,ringkasan,perubahan"
    );
    assert!(
        lines[1].starts_with("2026-11-18T00:31:00+07:00,"),
        "{}",
        lines[1]
    );
    assert!(
        lines[2].starts_with("2026-11-18T00:30:00+07:00,"),
        "{}",
        lines[2]
    );
    assert!(lines[1].contains("second"));
}

#[sqlx::test]
async fn csv_neutralises_formulas(pool: PgPool) {
    let h = Harness::new(pool).await;
    let session = h.reader().await;
    h.entry(base(), None, "update", r#"=HYPERLINK("http://x")"#)
        .await;

    let reply = h.get("/api/admin/audit.csv", &session).await;
    let mut reader = csv::Reader::from_reader(reply.text.as_bytes());
    let record = reader.records().next().unwrap().unwrap();
    assert_eq!(&record[5], r#"'=HYPERLINK("http://x")"#);
}

#[sqlx::test]
async fn audit_routes_need_audit_read(pool: PgPool) {
    let h = Harness::new(pool).await;
    let role = h.role("Penulis", &["content.write"]).await;
    h.user("penulis", "Penulis", role).await;
    let session = h.login("penulis").await;
    for uri in ["/api/admin/audit", "/api/admin/audit.csv"] {
        let reply = h.get(uri, &session).await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{uri}");
        assert_eq!(reply.json()["code"], "permission_denied", "{uri}");
    }
}

#[sqlx::test]
async fn range_invalid_is_422(pool: PgPool) {
    let h = Harness::new(pool).await;
    let session = h.reader().await;
    for uri in [
        "/api/admin/audit?from=2026-11-20&to=2026-11-18",
        "/api/admin/audit.csv?from=2026-11-20&to=2026-11-18",
    ] {
        let reply = h.get(uri, &session).await;
        assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY, "{uri}");
        assert_eq!(
            reply.json()["errors"],
            json!([{"field": "from", "code": "range_invalid"}])
        );
    }
}
