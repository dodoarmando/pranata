mod common;

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use common::{administrator_role, test_state_with_clock};
use http_body_util::BodyExt;
use pranata::{access::Permission, auth, ids::RoleId};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

const PASSWORD: &str = "correct horse battery";

struct Reply {
    status: StatusCode,
    body: Value,
}

struct Harness {
    pool: PgPool,
    app: Router,
    clock: std::sync::Arc<common::FixedClock>,
}

impl Harness {
    fn new(pool: PgPool) -> Self {
        let clock = common::FixedClock::new();
        let app = pranata::app(test_state_with_clock(pool.clone(), clock.clone()));
        Self { pool, app, clock }
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

    async fn user(&self, username: &str, role: RoleId) {
        let hash = auth::password::hash_password(PASSWORD).unwrap();
        sqlx::query!(
            "INSERT INTO users (username, display_name, password_hash, role_id)
             VALUES ($1, 'Budi Santoso', $2, $3)",
            username,
            hash,
            role.0
        )
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn send(&self, request: Request<Body>) -> (Reply, Option<String>) {
        let response = self.app.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let cookie = parts.headers.get(header::SET_COOKIE).map(|value| {
            value
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_owned()
        });
        let bytes = body.collect().await.unwrap().to_bytes();
        let reply = Reply {
            status: parts.status,
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        };
        (reply, cookie)
    }

    /// Sign in and return the cookie and CSRF token.
    async fn login(&self, username: &str) -> (String, String) {
        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"username": username, "password": PASSWORD}).to_string(),
            ))
            .unwrap();
        let (reply, cookie) = self.send(request).await;
        assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
        (
            cookie.unwrap(),
            reply.body["csrf_token"].as_str().unwrap().to_owned(),
        )
    }

    async fn me(&self, cookie: &str) -> Reply {
        let request = Request::builder()
            .uri("/api/auth/me")
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .unwrap();
        self.send(request).await.0
    }
}

#[sqlx::test]
async fn catalogue_matches_the_database_check(pool: PgPool) {
    let h = Harness::new(pool);
    let all: Vec<&str> = Permission::ALL.iter().map(|p| p.as_str()).collect();
    h.role("Semua", &all).await;

    let role = h.role("Lain", &[]).await;
    let rejected = sqlx::query!(
        "INSERT INTO role_permissions (role_id, permission) VALUES ($1, 'resident.read')",
        role.0
    )
    .execute(&h.pool)
    .await;
    assert!(rejected.is_err());

    for permission in Permission::ALL {
        assert_eq!(permission.as_str().parse::<Permission>(), Ok(permission));
        assert_eq!(
            serde_json::to_value(permission).unwrap(),
            json!(permission.as_str())
        );
    }
    assert!("resident.read".parse::<Permission>().is_err());
}

#[sqlx::test]
async fn administrators_hold_every_permission(pool: PgPool) {
    let h = Harness::new(pool);
    let role = administrator_role(&h.pool).await;
    let created = auth::create_user_with_access_link(
        &h.pool,
        "siti",
        "Siti",
        role,
        None,
        h.clock.now_for_test(),
    )
    .await
    .unwrap();
    let redeem = Request::builder()
        .method(Method::POST)
        .uri("/api/auth/access-links/redeem")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"token": created.access_link_token, "password": PASSWORD}).to_string(),
        ))
        .unwrap();
    assert_eq!(h.send(redeem).await.0.status, StatusCode::NO_CONTENT);

    let (cookie, _) = h.login("siti").await;
    let me = h.me(&cookie).await;
    let expected: Vec<&str> = Permission::ALL.iter().map(|p| p.as_str()).collect();
    assert_eq!(me.body["user"]["permissions"], json!(expected));
}

#[sqlx::test]
async fn users_need_a_role(pool: PgPool) {
    let rejected =
        sqlx::query!("INSERT INTO users (username, display_name) VALUES ('baru', 'Baru')")
            .execute(&pool)
            .await;
    assert!(rejected.is_err());
}

#[sqlx::test]
async fn administrator_role_is_unique(pool: PgPool) {
    let rejected =
        sqlx::query!("INSERT INTO roles (name, is_administrator) VALUES ('Kedua', true)")
            .execute(&pool)
            .await;
    assert!(rejected.is_err());
}

#[sqlx::test]
async fn role_names_are_unique_ignoring_case(pool: PgPool) {
    let rejected = sqlx::query!("INSERT INTO roles (name) VALUES ('administrator')")
        .execute(&pool)
        .await;
    assert!(rejected.is_err());
}

#[sqlx::test]
async fn me_lists_role_and_permissions(pool: PgPool) {
    let h = Harness::new(pool);
    let role = administrator_role(&h.pool).await;
    h.user("budi", role).await;
    let (cookie, _) = h.login("budi").await;

    let me = h.me(&cookie).await;
    assert_eq!(me.body["user"]["role"]["name"], "Administrator");
    assert_eq!(me.body["user"]["role"]["is_administrator"], true);
    assert_eq!(me.body["user"]["role"]["id"], json!(role.0));
}

#[sqlx::test]
async fn permissions_follow_the_catalogue_order_and_edits_apply_at_once(pool: PgPool) {
    let h = Harness::new(pool);
    let role = h
        .role("Editor", &["content.publish", "content.write"])
        .await;
    h.user("budi", role).await;
    let (cookie, _) = h.login("budi").await;

    let me = h.me(&cookie).await;
    assert_eq!(
        me.body["user"]["permissions"],
        json!(["content.write", "content.publish"])
    );
    assert_eq!(me.body["user"]["role"]["is_administrator"], false);

    sqlx::query!("DELETE FROM role_permissions WHERE role_id = $1", role.0)
        .execute(&h.pool)
        .await
        .unwrap();
    let me = h.me(&cookie).await;
    assert_eq!(me.body["user"]["permissions"], json!([]));
}
