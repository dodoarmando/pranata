mod common;

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use common::{FixedClock, test_state_with_clock};
use http_body_util::BodyExt;
use pranata::{
    auth,
    ids::{RoleId, UserId},
};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::Duration;
use tower::ServiceExt;

const PASSWORD: &str = "correct horse battery";

const ALL_PERMISSIONS: [&str; 7] = [
    "village.manage",
    "content.write",
    "content.publish",
    "budget.manage",
    "audit.read",
    "user.manage",
    "role.manage",
];

struct Harness {
    pool: PgPool,
    clock: Arc<FixedClock>,
    app: Router,
    admin_role: RoleId,
    next_ip: AtomicU32,
}

struct Reply {
    status: StatusCode,
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
        format!("10.1.{}.{}", (n >> 8) & 255, n & 255)
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

    /// A signed-in User holding `role`.
    async fn signed_in(&self, username: &str, role: RoleId) -> Session {
        self.user(username, role).await;
        self.login(username).await
    }

    async fn send(&self, request: Request<Body>) -> Reply {
        let response = self.app.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        Reply {
            status: parts.status,
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        }
    }

    async fn login(&self, username: &str) -> Session {
        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/auth/login")
            .header("x-forwarded-for", self.fresh_ip())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"username": username, "password": PASSWORD}).to_string(),
            ))
            .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let cookie = response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(status, StatusCode::OK, "{body}");
        Session {
            cookie,
            csrf: body["csrf_token"].as_str().unwrap().to_owned(),
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

    async fn list(&self, session: &Session) -> Reply {
        self.call(Method::GET, "/api/admin/roles", session, None)
            .await
    }

    async fn create(&self, session: &Session, name: &str, permissions: &[&str]) -> Reply {
        self.call(
            Method::POST,
            "/api/admin/roles",
            session,
            Some(json!({"name": name, "permissions": permissions})),
        )
        .await
    }

    async fn update(
        &self,
        session: &Session,
        role: RoleId,
        name: &str,
        permissions: &[&str],
    ) -> Reply {
        self.call(
            Method::PUT,
            &format!("/api/admin/roles/{}", role.0),
            session,
            Some(json!({"name": name, "permissions": permissions})),
        )
        .await
    }

    async fn delete(&self, session: &Session, role: RoleId) -> Reply {
        self.call(
            Method::DELETE,
            &format!("/api/admin/roles/{}", role.0),
            session,
            None,
        )
        .await
    }

    async fn role_exists(&self, role: RoleId) -> bool {
        sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM roles WHERE id = $1) AS "exists!""#,
            role.0
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// Audit Entries about Roles.
    async fn role_changes(&self) -> i64 {
        sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM audit_entries WHERE object_type = 'role'"#
        )
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

fn assert_problem(reply: &Reply, status: StatusCode, code: &str) {
    assert_eq!(reply.status, status, "{}", reply.body);
    assert_eq!(reply.body["code"], code, "{}", reply.body);
}

fn role_id(reply: &Reply) -> RoleId {
    RoleId(reply.body["id"].as_str().unwrap().parse().unwrap())
}

#[sqlx::test]
async fn create_update_delete_role_with_audit(pool: PgPool) {
    let h = Harness::new(pool).await;
    let admin = h.user("admin", h.admin_role).await;
    let session = h.login("admin").await;

    let created = h
        .create(
            &session,
            " Operator ",
            &["content.write", "village.manage", "content.write"],
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let id = role_id(&created);
    assert_eq!(
        created.body,
        json!({
            "id": id.0,
            "name": "Operator",
            "is_administrator": false,
            "permissions": ["village.manage", "content.write"],
            "user_count": 0,
        })
    );

    let updated = h
        .update(
            &session,
            id,
            "Operator Desa",
            &["audit.read", "content.write"],
        )
        .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    assert_eq!(updated.body["name"], "Operator Desa");
    assert_eq!(
        updated.body["permissions"],
        json!(["content.write", "audit.read"])
    );
    // Saving the same values again changes nothing and records nothing.
    let unchanged = h
        .update(
            &session,
            id,
            "Operator Desa",
            &["content.write", "audit.read"],
        )
        .await;
    assert_eq!(unchanged.status, StatusCode::OK, "{}", unchanged.body);

    let deleted = h.delete(&session, id).await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT, "{}", deleted.body);
    assert!(!h.role_exists(id).await);
    let orphans = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM role_permissions WHERE role_id = $1"#,
        id.0
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(orphans, 0);

    let entries = sqlx::query!(
        r#"SELECT actor, action, object_id, changes FROM audit_entries
           WHERE object_type = 'role' ORDER BY at, id"#
    )
    .fetch_all(&h.pool)
    .await
    .unwrap();
    let actions: Vec<&str> = entries.iter().map(|entry| entry.action.as_str()).collect();
    assert_eq!(actions, ["create", "update", "delete"]);
    for entry in &entries {
        assert_eq!(entry.actor, Some(admin.0));
        assert_eq!(entry.object_id, id.0.to_string());
    }
    assert_eq!(
        entries[1].changes,
        json!([
            {"field": "name", "before": "Operator", "after": "Operator Desa"},
            {
                "field": "permissions",
                "before": ["village.manage", "content.write"],
                "after": ["content.write", "audit.read"],
            },
        ])
    );
}

#[sqlx::test]
async fn listing_puts_the_administrator_first_and_counts_active_users(pool: PgPool) {
    let h = Harness::new(pool).await;
    let session = h.signed_in("admin", h.admin_role).await;
    let zeta = h.role("Zeta", &["content.write"]).await;
    let alpha = h.role("alpha", &["user.manage", "village.manage"]).await;
    h.user("andi", alpha).await;
    let budi = h.user("budi", alpha).await;
    sqlx::query!("UPDATE users SET is_active = false WHERE id = $1", budi.0)
        .execute(&h.pool)
        .await
        .unwrap();

    let list = h.list(&session).await;
    assert_eq!(list.status, StatusCode::OK, "{}", list.body);
    assert_eq!(
        list.body,
        json!([
            {
                "id": h.admin_role.0,
                "name": "Administrator",
                "is_administrator": true,
                "permissions": ALL_PERMISSIONS,
                "user_count": 1,
            },
            {
                "id": alpha.0,
                "name": "alpha",
                "is_administrator": false,
                "permissions": ["village.manage", "user.manage"],
                "user_count": 1,
            },
            {
                "id": zeta.0,
                "name": "Zeta",
                "is_administrator": false,
                "permissions": ["content.write"],
                "user_count": 0,
            },
        ])
    );

    // The Users screen needs the list for its Role picker.
    let managers = h.role("Pengelola", &["user.manage"]).await;
    let manager = h.signed_in("manajer", managers).await;
    assert_eq!(h.list(&manager).await.status, StatusCode::OK);
}

#[sqlx::test]
async fn administrator_role_is_read_only(pool: PgPool) {
    let h = Harness::new(pool).await;
    let session = h.signed_in("admin", h.admin_role).await;

    let renamed = h.update(&session, h.admin_role, "Kepala", &[]).await;
    assert_problem(&renamed, StatusCode::CONFLICT, "role_builtin");
    let deleted = h.delete(&session, h.admin_role).await;
    assert_problem(&deleted, StatusCode::CONFLICT, "role_builtin");

    // Anyone else is stopped earlier, by the escalation rules.
    let managers = h.role("Pengelola", &["role.manage"]).await;
    let manager = h.signed_in("manajer", managers).await;
    let renamed = h.update(&manager, h.admin_role, "Kepala", &[]).await;
    assert_problem(&renamed, StatusCode::FORBIDDEN, "administrator_only");
    let deleted = h.delete(&manager, h.admin_role).await;
    assert_problem(&deleted, StatusCode::FORBIDDEN, "administrator_only");

    assert!(h.role_exists(h.admin_role).await);
    assert_eq!(h.role_changes().await, 0);
}

#[sqlx::test]
async fn role_in_use_cannot_be_deleted(pool: PgPool) {
    let h = Harness::new(pool).await;
    let session = h.signed_in("admin", h.admin_role).await;
    let writers = h.role("Penulis", &["content.write"]).await;
    let budi = h.user("budi", writers).await;

    let refused = h.delete(&session, writers).await;
    assert_problem(&refused, StatusCode::CONFLICT, "role_in_use");

    // A deactivated User still holds the Role and may come back.
    sqlx::query!("UPDATE users SET is_active = false WHERE id = $1", budi.0)
        .execute(&h.pool)
        .await
        .unwrap();
    let refused = h.delete(&session, writers).await;
    assert_problem(&refused, StatusCode::CONFLICT, "role_in_use");
    assert!(h.role_exists(writers).await);
    assert_eq!(h.role_changes().await, 0);
}

/// Wait until a connection to this test's database waits for a row lock.
async fn wait_for_a_lock_wait(pool: &PgPool) {
    for _ in 0..500 {
        let waiting = sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM pg_stat_activity
               WHERE datname = current_database() AND wait_event_type = 'Lock'"#
        )
        .fetch_one(pool)
        .await
        .unwrap();
        if waiting > 0 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("no request waited for the lock");
}

#[sqlx::test]
async fn delete_waits_for_a_user_who_is_taking_the_role(pool: PgPool) {
    let h = Harness::new(pool).await;
    let session = h.signed_in("admin", h.admin_role).await;
    let writers = h.role("Penulis", &["content.write"]).await;

    // A User is being given the Role, not yet committed. The delete must wait
    // for that transaction and then see the Role in use.
    let mut assigning = h.pool.begin().await.unwrap();
    sqlx::query!(
        "INSERT INTO users (username, display_name, role_id) VALUES ('siti', 'Siti', $1)",
        writers.0
    )
    .execute(&mut *assigning)
    .await
    .unwrap();
    let (reply, ()) = tokio::join!(h.delete(&session, writers), async {
        wait_for_a_lock_wait(&h.pool).await;
        assigning.commit().await.unwrap();
    });
    assert_problem(&reply, StatusCode::CONFLICT, "role_in_use");
    assert!(h.role_exists(writers).await);
}

#[sqlx::test]
async fn role_names_are_unique_ignoring_case(pool: PgPool) {
    let h = Harness::new(pool).await;
    let session = h.signed_in("admin", h.admin_role).await;

    let operator = h.create(&session, "Operator", &[]).await;
    assert_eq!(operator.status, StatusCode::CREATED, "{}", operator.body);
    let again = h.create(&session, "operator ", &[]).await;
    assert_problem(&again, StatusCode::CONFLICT, "role_name_taken");
    let builtin = h.create(&session, "ADMINISTRATOR", &[]).await;
    assert_problem(&builtin, StatusCode::CONFLICT, "role_name_taken");

    let other = h.create(&session, "Lain", &[]).await;
    let renamed = h.update(&session, role_id(&other), "OPERATOR", &[]).await;
    assert_problem(&renamed, StatusCode::CONFLICT, "role_name_taken");
    // A Role may change the case of its own name.
    let recased = h
        .update(&session, role_id(&operator), "OPERATOR", &[])
        .await;
    assert_eq!(recased.status, StatusCode::OK, "{}", recased.body);
    assert_eq!(recased.body["name"], "OPERATOR");
}

#[sqlx::test]
async fn invalid_role_requests(pool: PgPool) {
    let h = Harness::new(pool).await;
    let session = h.signed_in("admin", h.admin_role).await;
    let name_length = json!([{"field": "name", "code": "name_length"}]);

    for name in ["   ".to_owned(), "x".repeat(51)] {
        let reply = h.create(&session, &name, &[]).await;
        assert_problem(
            &reply,
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation_failed",
        );
        assert_eq!(reply.body["errors"], name_length);
    }
    // Fifty characters, not bytes.
    let longest = h.create(&session, &"é".repeat(50), &[]).await;
    assert_eq!(longest.status, StatusCode::CREATED, "{}", longest.body);
    let blank = h.update(&session, role_id(&longest), " ", &[]).await;
    assert_problem(
        &blank,
        StatusCode::UNPROCESSABLE_ENTITY,
        "validation_failed",
    );
    assert_eq!(blank.body["errors"], name_length);

    let unknown = h.create(&session, "Peran", &["village.destroy"]).await;
    assert_problem(&unknown, StatusCode::BAD_REQUEST, "invalid_json");

    let nobody = RoleId(uuid::Uuid::new_v4());
    for reply in [
        h.update(&session, nobody, "Peran", &[]).await,
        h.delete(&session, nobody).await,
        h.call(
            Method::DELETE,
            "/api/admin/roles/not-a-uuid",
            &session,
            None,
        )
        .await,
    ] {
        assert_problem(&reply, StatusCode::NOT_FOUND, "role_not_found");
    }
    assert_eq!(h.role_changes().await, 1);
}

#[sqlx::test]
async fn role_change_applies_to_signed_in_users(pool: PgPool) {
    let h = Harness::new(pool).await;
    let admin = h.signed_in("admin", h.admin_role).await;
    let editors = h.role("Penyunting", &["content.write"]).await;
    let editor = h.signed_in("editor", editors).await;

    let refused = h
        .call(
            Method::PUT,
            "/api/admin/village/profile",
            &editor,
            Some(json!({})),
        )
        .await;
    assert_problem(&refused, StatusCode::FORBIDDEN, "permission_denied");

    let granted = h
        .update(
            &admin,
            editors,
            "Penyunting",
            &["content.write", "village.manage"],
        )
        .await;
    assert_eq!(granted.status, StatusCode::OK, "{}", granted.body);
    // The same cookie: the Permission check now passes, and the empty body
    // is what is refused.
    let passed = h
        .call(
            Method::PUT,
            "/api/admin/village/profile",
            &editor,
            Some(json!({})),
        )
        .await;
    assert_ne!(passed.status, StatusCode::FORBIDDEN, "{}", passed.body);
}

#[sqlx::test]
async fn role_routes_need_role_manage_and_sudo(pool: PgPool) {
    let h = Harness::new(pool).await;
    let writers = h.role("Penulis", &["content.write"]).await;
    let writer = h.signed_in("penulis", writers).await;
    for reply in [
        h.list(&writer).await,
        h.create(&writer, "Peran", &[]).await,
        h.update(&writer, writers, "Peran", &[]).await,
        h.delete(&writer, writers).await,
    ] {
        assert_problem(&reply, StatusCode::FORBIDDEN, "permission_denied");
    }

    let managers = h.role("Pengelola", &["role.manage"]).await;
    let manager = h.signed_in("manajer", managers).await;
    let target = h.role("Sasaran", &[]).await;
    h.clock.advance(Duration::minutes(11));
    for reply in [
        h.create(&manager, "Peran", &[]).await,
        h.update(&manager, target, "Peran", &[]).await,
        h.delete(&manager, target).await,
    ] {
        assert_problem(&reply, StatusCode::FORBIDDEN, "sudo_required");
    }
    assert_eq!(h.list(&manager).await.status, StatusCode::OK);
    assert_eq!(h.role_changes().await, 0);
}

#[sqlx::test]
async fn role_manager_cannot_grant_self_user_manage(pool: PgPool) {
    let h = Harness::new(pool).await;
    let managers = h.role("Pengelola", &["role.manage"]).await;
    let manager = h.signed_in("manajer", managers).await;

    let own = h
        .update(
            &manager,
            managers,
            "Pengelola",
            &["role.manage", "user.manage"],
        )
        .await;
    assert_problem(&own, StatusCode::FORBIDDEN, "own_role");
    let own = h.delete(&manager, managers).await;
    assert_problem(&own, StatusCode::FORBIDDEN, "own_role");

    let granted = h.create(&manager, "Pengguna", &["user.manage"]).await;
    assert_problem(&granted, StatusCode::FORBIDDEN, "permission_not_held");
    let mixed = h
        .create(&manager, "Campuran", &["role.manage", "user.manage"])
        .await;
    assert_problem(&mixed, StatusCode::FORBIDDEN, "permission_not_held");
    assert_eq!(h.role_changes().await, 0);

    let created = h.create(&manager, "Wakil", &["role.manage"]).await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let widened = h
        .update(&manager, role_id(&created), "Wakil", &["user.manage"])
        .await;
    assert_problem(&widened, StatusCode::FORBIDDEN, "permission_not_held");
}

#[sqlx::test]
async fn non_admin_cannot_edit_or_delete_a_stronger_role(pool: PgPool) {
    let h = Harness::new(pool).await;
    let managers = h.role("Pengelola", &["role.manage", "content.write"]).await;
    let manager = h.signed_in("manajer", managers).await;
    let secretaries = h
        .role("Sekretaris", &["village.manage", "content.write"])
        .await;

    // Taking village.manage away from the Sekretaris Role would strip its
    // holders of a Permission the manager cannot grant back.
    let narrowed = h
        .update(&manager, secretaries, "Sekretaris", &["content.write"])
        .await;
    assert_problem(&narrowed, StatusCode::FORBIDDEN, "permission_not_held");
    let deleted = h.delete(&manager, secretaries).await;
    assert_problem(&deleted, StatusCode::FORBIDDEN, "permission_not_held");
    assert_eq!(h.role_changes().await, 0);

    let writers = h.role("Penulis", &["content.write"]).await;
    let narrowed = h.update(&manager, writers, "Penulis Lepas", &[]).await;
    assert_eq!(narrowed.status, StatusCode::OK, "{}", narrowed.body);
    let deleted = h.delete(&manager, writers).await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT, "{}", deleted.body);
}
