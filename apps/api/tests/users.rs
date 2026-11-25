mod common;

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode, header},
};
use common::{FixedClock, test_state_with_clock};
use http_body_util::BodyExt;
use pranata::{
    auth::{self, access_link},
    ids::{RoleId, UserId},
};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::Duration;
use tower::ServiceExt;

const PASSWORD: &str = "correct horse battery";

struct Harness {
    pool: PgPool,
    clock: Arc<FixedClock>,
    app: Router,
    admin_role: RoleId,
    next_ip: AtomicU32,
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
}

/// A signed-in browser: the session cookie and its CSRF token.
struct Session {
    cookie: String,
    csrf: String,
}

impl Harness {
    async fn new(pool: PgPool) -> Self {
        let clock = FixedClock::new();
        let app = pranata::app(test_state_with_clock(pool.clone(), clock.clone()));
        let admin_role = common::administrator_role(&pool).await;
        Self {
            pool,
            clock,
            app,
            admin_role,
            next_ip: AtomicU32::new(0),
        }
    }

    /// A client address nobody used before, so the per-IP limit on sign-in
    /// never interferes.
    fn fresh_ip(&self) -> String {
        let n = self.next_ip.fetch_add(1, Ordering::Relaxed);
        format!("10.0.{}.{}", (n >> 8) & 255, n & 255)
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

    /// A User with a password, so they can sign in.
    async fn user(&self, username: &str, role: RoleId) -> UserId {
        let hash = auth::password::hash_password(PASSWORD).unwrap();
        sqlx::query_scalar!(
            r#"INSERT INTO users (username, display_name, password_hash, role_id)
               VALUES ($1, $1, $2, $3) RETURNING id AS "id: UserId""#,
            username,
            hash,
            role.0
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// An Administrator with a password, signed in.
    async fn admin(&self, username: &str) -> (UserId, Session) {
        let id = self.user(username, self.admin_role).await;
        (id, self.login(username).await)
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

    async fn try_login(&self, username: &str, password: &str) -> (Reply, Option<String>) {
        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/auth/login")
            .header("x-forwarded-for", self.fresh_ip())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"username": username, "password": password}).to_string(),
            ))
            .unwrap();
        let reply = self.send(request).await;
        let cookie = reply.headers.get(header::SET_COOKIE).map(|value| {
            value
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_owned()
        });
        (reply, cookie)
    }

    async fn login(&self, username: &str) -> Session {
        let (reply, cookie) = self.try_login(username, PASSWORD).await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
        Session {
            cookie: cookie.unwrap(),
            csrf: reply.body["csrf_token"].as_str().unwrap().to_owned(),
        }
    }

    async fn call(
        &self,
        method: Method,
        uri: &str,
        session: &Session,
        body: Option<Value>,
    ) -> Reply {
        let builder = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::COOKIE, &session.cookie)
            .header("x-csrf-token", &session.csrf);
        let request = match body {
            Some(body) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => builder.body(Body::empty()),
        }
        .unwrap();
        self.send(request).await
    }

    async fn me(&self, session: &Session) -> StatusCode {
        let request = Request::builder()
            .uri("/api/auth/me")
            .header(header::COOKIE, &session.cookie)
            .body(Body::empty())
            .unwrap();
        self.send(request).await.status
    }

    async fn redeem(&self, token: &str) -> Reply {
        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/auth/access-links/redeem")
            .header("x-forwarded-for", self.fresh_ip())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"token": token, "password": PASSWORD}).to_string(),
            ))
            .unwrap();
        self.send(request).await
    }

    async fn create_user(&self, session: &Session, username: &str, role: RoleId) -> Reply {
        self.call(
            Method::POST,
            "/api/admin/users",
            session,
            Some(json!({"username": username, "display_name": "Siti Aminah", "role_id": role.0})),
        )
        .await
    }

    async fn patch(&self, session: &Session, user: UserId, body: Value) -> Reply {
        self.call(
            Method::PATCH,
            &format!("/api/admin/users/{}", user.0),
            session,
            Some(body),
        )
        .await
    }

    async fn post(&self, session: &Session, user: UserId, action: &str) -> Reply {
        self.call(
            Method::POST,
            &format!("/api/admin/users/{}/{action}", user.0),
            session,
            None,
        )
        .await
    }

    async fn active_administrators(&self) -> i64 {
        sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM users WHERE is_active AND role_id = $1"#,
            self.admin_role.0
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// Audit Entries about Users other than sign-ins.
    async fn user_changes(&self) -> i64 {
        sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM audit_entries
               WHERE object_type = 'user' AND action <> 'sign_in'"#
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

/// The token in an Access Link URL, after checking the URL's shape.
fn token_of(url: &Value) -> String {
    let url = url.as_str().unwrap();
    let token = url
        .strip_prefix("http://localhost:3000/akses#")
        .unwrap_or_else(|| panic!("unexpected access link url {url}"));
    token.to_owned()
}

fn assert_problem(reply: &Reply, status: StatusCode, code: &str) {
    assert_eq!(reply.status, status, "{}", reply.body);
    assert_eq!(reply.body["code"], code, "{}", reply.body);
}

#[sqlx::test]
async fn creating_a_user_returns_a_working_access_link(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (admin, session) = h.admin("admin").await;
    let editor = h.role("Penulis", &["content.write"]).await;

    let created = h.create_user(&session, " Siti ", editor).await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let user = &created.body["user"];
    assert_eq!(user["username"], "siti");
    assert_eq!(user["display_name"], "Siti Aminah");
    assert_eq!(user["role"]["id"], json!(editor.0));
    assert_eq!(user["role"]["name"], "Penulis");
    assert_eq!(user["is_active"], true);
    assert_eq!(user["has_password"], false);
    assert_eq!(user["passkey_count"], 0);
    let expires_at = created.body["access_link"]["expires_at"].as_str().unwrap();
    let expected = h.clock.now_for_test() + Duration::hours(24);
    assert_eq!(
        time::OffsetDateTime::parse(expires_at, &time::format_description::well_known::Rfc3339)
            .unwrap(),
        expected
    );
    let token = token_of(&created.body["access_link"]["url"]);

    let entry = sqlx::query!(
        r#"SELECT actor, object_id, to_jsonb(a)::text AS "row!" FROM audit_entries a
           WHERE action = 'create' AND object_type = 'user'"#
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(entry.actor, Some(admin.0));
    assert_eq!(entry.object_id, user["id"].as_str().unwrap());
    assert!(!entry.row.contains(&token), "{}", entry.row);
    assert!(!entry.row.contains("akses"), "{}", entry.row);

    assert_eq!(h.redeem(&token).await.status, StatusCode::NO_CONTENT);
    h.login("siti").await;
}

#[sqlx::test]
async fn user_routes_need_user_manage(pool: PgPool) {
    let h = Harness::new(pool).await;
    let writer = h.role("Penulis", &["content.write"]).await;
    let me = h.user("budi", writer).await;
    let session = h.login("budi").await;

    let list = h
        .call(Method::GET, "/api/admin/users", &session, None)
        .await;
    assert_problem(&list, StatusCode::FORBIDDEN, "permission_denied");
    for reply in [
        h.create_user(&session, "siti", writer).await,
        h.patch(&session, me, json!({"display_name": "Budi"})).await,
        h.post(&session, me, "deactivate").await,
        h.post(&session, me, "reactivate").await,
        h.post(&session, me, "access-links").await,
    ] {
        assert_problem(&reply, StatusCode::FORBIDDEN, "permission_denied");
    }
}

#[sqlx::test]
async fn writes_need_sudo(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, session) = h.admin("admin").await;
    h.clock.advance(Duration::minutes(11));

    let created = h.create_user(&session, "siti", h.admin_role).await;
    assert_problem(&created, StatusCode::FORBIDDEN, "sudo_required");
    let list = h
        .call(Method::GET, "/api/admin/users", &session, None)
        .await;
    assert_eq!(list.status, StatusCode::OK);
}

#[sqlx::test]
async fn listing_puts_active_users_first_then_sorts_by_name(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, session) = h.admin("zaenal").await;
    let writer = h.role("Penulis", &["content.write"]).await;
    let andi = h.user("andi", writer).await;
    h.user("budi", writer).await;
    h.post(&session, andi, "deactivate").await;

    let list = h
        .call(Method::GET, "/api/admin/users", &session, None)
        .await;
    assert_eq!(list.status, StatusCode::OK);
    let names: Vec<&str> = list
        .body
        .as_array()
        .unwrap()
        .iter()
        .map(|user| user["username"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["budi", "zaenal", "andi"]);
    let budi = &list.body[0];
    assert_eq!(budi["has_password"], true);
    assert_eq!(budi["is_active"], true);
    assert_eq!(budi["role"]["name"], "Penulis");
    assert_eq!(budi["role"]["is_administrator"], false);
    assert_eq!(list.body[2]["is_active"], false);
}

#[sqlx::test]
async fn deactivation_ends_sessions_and_links(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (admin, session) = h.admin("admin").await;
    let writer = h.role("Penulis", &["content.write"]).await;
    let budi = h.user("budi", writer).await;
    let budi_session = h.login("budi").await;
    let issued = h.post(&session, budi, "access-links").await;
    assert_eq!(issued.status, StatusCode::CREATED, "{}", issued.body);
    let token = token_of(&issued.body["url"]);

    let reply = h.post(&session, budi, "deactivate").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.body["is_active"], false);
    assert_eq!(h.me(&budi_session).await, StatusCode::UNAUTHORIZED);
    assert_problem(
        &h.redeem(&token).await,
        StatusCode::GONE,
        "access_link_revoked",
    );

    let sessions = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM sessions WHERE user_id = $1"#,
        budi.0
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(sessions, 0);
    let revoked = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM access_links
           WHERE user_id = $1 AND revoked_at IS NOT NULL"#,
        budi.0
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(revoked, 1);
    let entry = sqlx::query!(
        "SELECT actor, object_id FROM audit_entries WHERE action = 'deactivate' AND object_type = 'user'"
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(entry.actor, Some(admin.0));
    assert_eq!(entry.object_id, budi.0.to_string());
}

#[sqlx::test]
async fn usernames_are_never_reused(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, session) = h.admin("admin").await;
    let writer = h.role("Penulis", &["content.write"]).await;
    let budi = h.user("budi", writer).await;
    assert_eq!(
        h.post(&session, budi, "deactivate").await.status,
        StatusCode::OK
    );

    let again = h.create_user(&session, "Budi", writer).await;
    assert_problem(&again, StatusCode::CONFLICT, "username_taken");
}

#[sqlx::test]
async fn self_service_limits(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (admin, session) = h.admin("admin").await;
    h.admin("kedua").await;
    let writer = h.role("Penulis", &["content.write"]).await;

    let deactivate = h.post(&session, admin, "deactivate").await;
    assert_problem(&deactivate, StatusCode::CONFLICT, "cannot_deactivate_self");
    let demote = h.patch(&session, admin, json!({"role_id": writer.0})).await;
    assert_problem(&demote, StatusCode::FORBIDDEN, "own_role");
    assert_eq!(h.active_administrators().await, 2);
    assert_eq!(h.user_changes().await, 0);
}

#[sqlx::test]
async fn concurrent_demotions_keep_one_administrator(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (a, a_session) = h.admin("andi").await;
    let (b, b_session) = h.admin("budi").await;
    let writer = h.role("Penulis", &["content.write"]).await;

    let (first, second) = tokio::join!(
        h.patch(&a_session, b, json!({"role_id": writer.0})),
        h.patch(&b_session, a, json!({"role_id": writer.0})),
    );
    let mut statuses = [first.status, second.status];
    statuses.sort();
    assert_eq!(
        statuses,
        [StatusCode::OK, StatusCode::CONFLICT],
        "{} / {}",
        first.body,
        second.body
    );
    let refused = if first.status == StatusCode::CONFLICT {
        &first
    } else {
        &second
    };
    assert_eq!(refused.body["code"], "last_administrator");
    assert_eq!(h.active_administrators().await, 1);
}

#[sqlx::test]
async fn concurrent_deactivations_keep_one_administrator(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (a, a_session) = h.admin("andi").await;
    let (b, b_session) = h.admin("budi").await;

    let (first, second) = tokio::join!(
        h.post(&a_session, b, "deactivate"),
        h.post(&b_session, a, "deactivate"),
    );
    let mut statuses = [first.status, second.status];
    statuses.sort();
    // The loser either finds no other Administrator (409) or, when the
    // winner committed before its session was read, is signed out (401).
    assert_eq!(
        statuses[0],
        StatusCode::OK,
        "{} / {}",
        first.body,
        second.body
    );
    assert!(
        [StatusCode::UNAUTHORIZED, StatusCode::CONFLICT].contains(&statuses[1]),
        "{} / {}",
        first.body,
        second.body
    );
    assert_eq!(h.active_administrators().await, 1);
}

#[sqlx::test]
async fn issuing_a_link_revokes_earlier_unused_links(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (admin, session) = h.admin("admin").await;
    let writer = h.role("Penulis", &["content.write"]).await;
    let budi = h.user("budi", writer).await;

    let first = h.post(&session, budi, "access-links").await;
    let second = h.post(&session, budi, "access-links").await;
    assert_eq!(first.status, StatusCode::CREATED, "{}", first.body);
    assert_eq!(second.status, StatusCode::CREATED, "{}", second.body);
    let (first, second) = (token_of(&first.body["url"]), token_of(&second.body["url"]));

    assert_problem(
        &h.redeem(&first).await,
        StatusCode::GONE,
        "access_link_revoked",
    );
    assert_eq!(h.redeem(&second).await.status, StatusCode::NO_CONTENT);

    let entries = sqlx::query!(
        r#"SELECT actor, object_id, to_jsonb(a)::text AS "row!" FROM audit_entries a
           WHERE action = 'issue_access_link'"#
    )
    .fetch_all(&h.pool)
    .await
    .unwrap();
    assert_eq!(entries.len(), 2);
    for entry in entries {
        assert_eq!(entry.actor, Some(admin.0));
        assert_eq!(entry.object_id, budi.0.to_string());
        assert!(!entry.row.contains(&first) && !entry.row.contains(&second));
        assert!(!entry.row.contains("akses"), "{}", entry.row);
    }
}

#[sqlx::test]
async fn inactive_users_get_no_access_link(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, session) = h.admin("admin").await;
    let writer = h.role("Penulis", &["content.write"]).await;
    let budi = h.user("budi", writer).await;
    h.post(&session, budi, "deactivate").await;

    let issued = h.post(&session, budi, "access-links").await;
    assert_problem(&issued, StatusCode::CONFLICT, "user_inactive");
}

#[sqlx::test]
async fn reactivated_user_can_sign_in_again(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (admin, session) = h.admin("admin").await;
    let writer = h.role("Penulis", &["content.write"]).await;
    let budi = h.user("budi", writer).await;
    assert_eq!(
        h.post(&session, budi, "deactivate").await.status,
        StatusCode::OK
    );
    let (refused, _) = h.try_login("budi", PASSWORD).await;
    assert_eq!(refused.status, StatusCode::UNAUTHORIZED);

    let reply = h.post(&session, budi, "reactivate").await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.body["is_active"], true);
    h.login("budi").await;

    let entry = sqlx::query!(
        "SELECT actor, object_id FROM audit_entries WHERE action = 'reactivate' AND object_type = 'user'"
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(entry.actor, Some(admin.0));
    assert_eq!(entry.object_id, budi.0.to_string());
}

#[sqlx::test]
async fn uncredentialed_administrator_does_not_count(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (a, a_session) = h.admin("andi").await;
    let (b, b_session) = h.admin("budi").await;
    let writer = h.role("Penulis", &["content.write"]).await;
    let created = h.create_user(&a_session, "dewi", h.admin_role).await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let dewi_token = token_of(&created.body["access_link"]["url"]);

    let (first, second) = tokio::join!(
        h.patch(&a_session, b, json!({"role_id": writer.0})),
        h.patch(&b_session, a, json!({"role_id": writer.0})),
    );
    let mut statuses = [first.status, second.status];
    statuses.sort();
    assert_eq!(
        statuses,
        [StatusCode::OK, StatusCode::CONFLICT],
        "{} / {}",
        first.body,
        second.body
    );
    let (loser, winner_session) = if first.status == StatusCode::OK {
        (b, &a_session)
    } else {
        (a, &b_session)
    };
    // Put the loser back, so both are Administrators again.
    let restored = h
        .patch(winner_session, loser, json!({"role_id": h.admin_role.0}))
        .await;
    assert_eq!(restored.status, StatusCode::OK, "{}", restored.body);
    assert_eq!(h.active_administrators().await, 3);

    assert_eq!(h.redeem(&dewi_token).await.status, StatusCode::NO_CONTENT);
    let (first, second) = tokio::join!(
        h.patch(&a_session, b, json!({"role_id": writer.0})),
        h.patch(&b_session, a, json!({"role_id": writer.0})),
    );
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);
    assert_eq!(second.status, StatusCode::OK, "{}", second.body);
    assert_eq!(h.active_administrators().await, 1);
}

#[sqlx::test]
async fn access_link_responses_are_not_cached(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, session) = h.admin("admin").await;
    let writer = h.role("Penulis", &["content.write"]).await;

    let created = h.create_user(&session, "siti", writer).await;
    assert_eq!(created.status, StatusCode::CREATED);
    assert_eq!(created.headers[header::CACHE_CONTROL], "no-store");
    let id = UserId(
        created.body["user"]["id"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap(),
    );
    let issued = h.post(&session, id, "access-links").await;
    assert_eq!(issued.status, StatusCode::CREATED);
    assert_eq!(issued.headers[header::CACHE_CONTROL], "no-store");
}

#[sqlx::test]
async fn non_admin_cannot_grant_administrator(pool: PgPool) {
    let h = Harness::new(pool).await;
    let managers = h.role("Pengelola", &["user.manage"]).await;
    let me = h.user("manajer", managers).await;
    let writer = h.role("Penulis", &["content.write"]).await;
    let budi = h.user("budi", writer).await;
    let session = h.login("manajer").await;

    let created = h.create_user(&session, "siti", h.admin_role).await;
    assert_problem(&created, StatusCode::FORBIDDEN, "administrator_only");
    for target in [me, budi] {
        let promoted = h
            .patch(&session, target, json!({"role_id": h.admin_role.0}))
            .await;
        assert_problem(&promoted, StatusCode::FORBIDDEN, "administrator_only");
    }
    assert_eq!(h.active_administrators().await, 0);
    assert_eq!(h.user_changes().await, 0);
}

#[sqlx::test]
async fn non_admin_cannot_touch_an_administrator(pool: PgPool) {
    let h = Harness::new(pool).await;
    let admin = h.user("admin", h.admin_role).await;
    let token = {
        let mut conn = h.pool.acquire().await.unwrap();
        access_link::create(&mut conn, admin, None, h.clock.now_for_test())
            .await
            .unwrap()
    };
    let managers = h.role("Pengelola", &["user.manage"]).await;
    h.user("manajer", managers).await;
    let session = h.login("manajer").await;

    for reply in [
        h.post(&session, admin, "deactivate").await,
        h.post(&session, admin, "reactivate").await,
        h.patch(&session, admin, json!({"display_name": "Bukan Admin"}))
            .await,
        h.patch(&session, admin, json!({"role_id": managers.0}))
            .await,
        h.post(&session, admin, "access-links").await,
    ] {
        assert_problem(&reply, StatusCode::FORBIDDEN, "administrator_only");
    }
    assert_eq!(h.user_changes().await, 0);
    let row = sqlx::query!(
        "SELECT display_name, is_active, role_id FROM users WHERE id = $1",
        admin.0
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(row.display_name, "admin");
    assert!(row.is_active);
    assert_eq!(row.role_id, h.admin_role.0);
    assert_eq!(h.redeem(&token).await.status, StatusCode::NO_CONTENT);
}

#[sqlx::test]
async fn non_admin_cannot_issue_link_for_administrator(pool: PgPool) {
    let h = Harness::new(pool).await;
    let admin = h.user("admin", h.admin_role).await;
    let managers = h.role("Pengelola", &["user.manage"]).await;
    h.user("manajer", managers).await;
    let session = h.login("manajer").await;

    let issued = h.post(&session, admin, "access-links").await;
    assert_problem(&issued, StatusCode::FORBIDDEN, "administrator_only");
    let links = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM access_links WHERE user_id = $1"#,
        admin.0
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(links, 0);
}

#[sqlx::test]
async fn non_admin_cannot_take_over_a_user_with_more_permissions(pool: PgPool) {
    let h = Harness::new(pool).await;
    let managers = h.role("Pengelola", &["user.manage"]).await;
    h.user("manajer", managers).await;
    let secretaries = h
        .role("Sekretaris", &["user.manage", "village.manage"])
        .await;
    let sekdes = h.user("sekdes", secretaries).await;
    let session = h.login("manajer").await;

    // A link for sekdes would let the manager set sekdes's password and sign
    // in with village.manage.
    for reply in [
        h.post(&session, sekdes, "access-links").await,
        h.post(&session, sekdes, "deactivate").await,
        h.patch(&session, sekdes, json!({"role_id": managers.0}))
            .await,
        h.patch(&session, sekdes, json!({"display_name": "Sekdes"}))
            .await,
        h.post(&session, sekdes, "reactivate").await,
    ] {
        assert_problem(&reply, StatusCode::FORBIDDEN, "stronger_user");
    }
    assert_eq!(h.user_changes().await, 0);
}

#[sqlx::test]
async fn nobody_changes_their_own_role(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (admin, session) = h.admin("admin").await;
    let writer = h.role("Penulis", &["content.write"]).await;

    let reply = h.patch(&session, admin, json!({"role_id": writer.0})).await;
    assert_problem(&reply, StatusCode::FORBIDDEN, "own_role");

    let managers = h.role("Pengelola", &["user.manage", "content.write"]).await;
    let me = h.user("manajer", managers).await;
    let session = h.login("manajer").await;
    let reply = h.patch(&session, me, json!({"role_id": writer.0})).await;
    assert_problem(&reply, StatusCode::FORBIDDEN, "own_role");
    // The name is not the Role: that stays allowed.
    let renamed = h
        .patch(&session, me, json!({"display_name": "Manajer"}))
        .await;
    assert_eq!(renamed.status, StatusCode::OK, "{}", renamed.body);
}

#[sqlx::test]
async fn non_admin_assigns_only_roles_within_their_permissions(pool: PgPool) {
    let h = Harness::new(pool).await;
    let managers = h.role("Pengelola", &["user.manage"]).await;
    h.user("manajer", managers).await;
    let village = h.role("Penata", &["village.manage"]).await;
    let session = h.login("manajer").await;

    let refused = h.create_user(&session, "siti", village).await;
    assert_problem(&refused, StatusCode::FORBIDDEN, "permission_not_held");
    let created = h.create_user(&session, "siti", managers).await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let siti = UserId(
        created.body["user"]["id"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap(),
    );
    let moved = h.patch(&session, siti, json!({"role_id": village.0})).await;
    assert_problem(&moved, StatusCode::FORBIDDEN, "permission_not_held");
}

#[sqlx::test]
async fn editing_records_what_changed(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (admin, session) = h.admin("admin").await;
    let writer = h.role("Penulis", &["content.write"]).await;
    let publisher = h.role("Penerbit", &["content.publish"]).await;
    let budi = h.user("budi", writer).await;

    let reply = h
        .patch(
            &session,
            budi,
            json!({"display_name": " Budi Santoso ", "role_id": publisher.0}),
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    assert_eq!(reply.body["username"], "budi");
    assert_eq!(reply.body["display_name"], "Budi Santoso");
    assert_eq!(reply.body["role"]["name"], "Penerbit");

    let entry = sqlx::query!(
        "SELECT actor, object_id, changes FROM audit_entries WHERE action = 'update' AND object_type = 'user'"
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(entry.actor, Some(admin.0));
    assert_eq!(entry.object_id, budi.0.to_string());
    assert_eq!(
        entry.changes,
        json!([
            {"field": "display_name", "before": "budi", "after": "Budi Santoso"},
            {"field": "role", "before": "Penulis", "after": "Penerbit"},
        ])
    );

    let renamed = h
        .patch(&session, budi, json!({"username": "budiman"}))
        .await;
    assert_eq!(renamed.status, StatusCode::BAD_REQUEST, "{}", renamed.body);
}

#[sqlx::test]
async fn invalid_requests_are_422_and_unknown_users_404(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, session) = h.admin("admin").await;
    let writer = h.role("Penulis", &["content.write"]).await;
    let budi = h.user("budi", writer).await;
    let unknown_role = RoleId(uuid::Uuid::new_v4());

    let created = h.create_user(&session, "siti", unknown_role).await;
    assert_problem(
        &created,
        StatusCode::UNPROCESSABLE_ENTITY,
        "validation_failed",
    );
    assert_eq!(
        created.body["errors"],
        json!([{"field": "role_id", "code": "role_not_found"}])
    );
    let blank = h
        .patch(
            &session,
            budi,
            json!({"display_name": "  ", "role_id": unknown_role.0}),
        )
        .await;
    assert_problem(
        &blank,
        StatusCode::UNPROCESSABLE_ENTITY,
        "validation_failed",
    );
    assert_eq!(
        blank.body["errors"],
        json!([
            {"field": "display_name", "code": "required"},
            {"field": "role_id", "code": "role_not_found"},
        ])
    );

    let nobody = UserId(uuid::Uuid::new_v4());
    for reply in [
        h.patch(&session, nobody, json!({"display_name": "Siapa"}))
            .await,
        h.post(&session, nobody, "deactivate").await,
        h.post(&session, nobody, "reactivate").await,
        h.post(&session, nobody, "access-links").await,
        h.call(
            Method::POST,
            "/api/admin/users/not-a-uuid/deactivate",
            &session,
            None,
        )
        .await,
    ] {
        assert_problem(&reply, StatusCode::NOT_FOUND, "user_not_found");
    }
}

#[sqlx::test]
async fn only_a_real_change_is_written(pool: PgPool) {
    let h = Harness::new(pool).await;
    let (_, session) = h.admin("admin").await;
    let writer = h.role("Penulis", &["content.write"]).await;
    let publisher = h.role("Penerbit", &["content.publish"]).await;
    let budi = h.user("budi", writer).await;

    let same = h
        .patch(
            &session,
            budi,
            json!({"display_name": "budi", "role_id": writer.0}),
        )
        .await;
    assert_eq!(same.status, StatusCode::OK, "{}", same.body);
    assert_eq!(h.user_changes().await, 0);

    let moved = h
        .patch(&session, budi, json!({"role_id": publisher.0}))
        .await;
    assert_eq!(moved.status, StatusCode::OK, "{}", moved.body);
    assert_eq!(moved.body["role"]["id"], json!(publisher.0));
    assert_eq!(h.user_changes().await, 1);
}
