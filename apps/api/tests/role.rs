mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::{Value, json};
use sqlx::PgPool;

const R1: [&str; 6] = [
    "village.manage",
    "content.write",
    "content.publish",
    "complaint.read",
    "complaint.handle",
    "audit.read",
];

/// The Roles as `{ name: permissions }`, with `"*"` for the Administrator's every Permission.
fn by_name(roles: &Value) -> Value {
    roles
        .as_array()
        .unwrap()
        .iter()
        .map(|role| {
            let permissions = if role["administrator"] == true {
                json!("*")
            } else {
                role["permissions"].clone()
            };
            (role["name"].as_str().unwrap().to_owned(), permissions)
        })
        .collect::<serde_json::Map<_, _>>()
        .into()
}

#[sqlx::test]
async fn the_administrator_and_the_editable_roles_are_seeded(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let cookie = app.sign_in("admin", false).await;

    let reply = app.call("GET", "/roles", None, Some(&cookie)).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        by_name(&reply.body),
        json!({
            "Administrator": "*",
            "Operator": R1[..5],
            "Kepala Desa": R1,
            "Kontributor Web": ["content.write", "content.publish"],
        })
    );
}

#[sqlx::test]
async fn only_the_administrator_role_manages_roles(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "kades", &R1).await;

    let reply = app.call("GET", "/roles", None, Some(&cookie)).await;

    assert_eq!(reply.status, StatusCode::FORBIDDEN);
}

async fn admin(app: &TestApp, pool: &PgPool) -> String {
    app.administrator(pool, "admin").await;
    app.sign_in("admin", false).await
}

async fn role_id(pool: &PgPool, name: &str) -> String {
    sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM roles WHERE name = $1")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
        .to_string()
}

async fn last_audit(pool: &PgPool) -> (String, Value, Value) {
    sqlx::query_as("SELECT action, details, changes FROM audit_entries ORDER BY id DESC LIMIT 1")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn the_administrator_composes_a_role_from_the_catalog(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;

    let reply = app
        .call(
            "POST",
            "/roles",
            Some(&json!({ "name": "Kaur Umum", "permissions": ["village.manage", "complaint.read"] })),
            Some(&cookie),
        )
        .await;

    assert_eq!(reply.status, StatusCode::CREATED);
    assert_eq!(reply.body["name"], "Kaur Umum");
    assert_eq!(reply.body["administrator"], false);
    assert_eq!(
        reply.body["permissions"],
        json!(["village.manage", "complaint.read"])
    );
    let roles = app.call("GET", "/roles", None, Some(&cookie)).await.body;
    assert_eq!(
        by_name(&roles)["Kaur Umum"],
        json!(["village.manage", "complaint.read"])
    );
    let (action, details, _) = last_audit(&pool).await;
    assert_eq!(action, "role.create");
    assert_eq!(
        details,
        json!({ "name": "Kaur Umum", "permissions": ["village.manage", "complaint.read"] })
    );
}

#[sqlx::test]
async fn a_role_needs_a_free_name_and_catalog_permissions(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;

    for (body, status) in [
        (
            json!({ "name": "Kaur", "permissions": ["user.manage"] }),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            json!({ "name": "  ", "permissions": [] }),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            json!({ "name": "Operator", "permissions": [] }),
            StatusCode::CONFLICT,
        ),
    ] {
        let reply = app.call("POST", "/roles", Some(&body), Some(&cookie)).await;
        assert_eq!(reply.status, status, "{body}");
    }
}

#[sqlx::test]
async fn a_role_edit_applies_on_the_next_request(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let kades = app.user_with(&pool, "kades", &["village.manage"]).await;
    let id = role_id(&pool, "kades").await;
    let audit_log = || app.call("GET", "/audit-entries", None, Some(&kades));
    assert_eq!(audit_log().await.status, StatusCode::FORBIDDEN);

    let reply = app
        .call(
            "PUT",
            &format!("/roles/{id}"),
            Some(&json!({ "name": "Kades", "permissions": ["village.manage", "audit.read"] })),
            Some(&cookie),
        )
        .await;

    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    assert_eq!(audit_log().await.status, StatusCode::OK);
    let (action, _, changes) = last_audit(&pool).await;
    assert_eq!(action, "role.update");
    assert_eq!(
        changes,
        json!([
            { "field": "name", "from": "kades", "to": "Kades" },
            { "field": "permissions", "from": ["village.manage"], "to": ["village.manage", "audit.read"] },
        ])
    );
}

#[sqlx::test]
async fn the_administrator_role_is_locked(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let id = role_id(&pool, "Administrator").await;

    let reply = app
        .call(
            "PUT",
            &format!("/roles/{id}"),
            Some(&json!({ "name": "Admin", "permissions": [] })),
            Some(&cookie),
        )
        .await;

    assert_eq!(reply.status, StatusCode::CONFLICT);
}

#[sqlx::test]
async fn editing_an_unknown_role_is_not_found(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;

    let reply = app
        .call(
            "PUT",
            &format!("/roles/{}", uuid::Uuid::nil()),
            Some(&json!({ "name": "X", "permissions": [] })),
            Some(&cookie),
        )
        .await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn only_the_administrator_role_creates_or_edits_roles(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(&pool, "kades", &R1).await;
    let id = role_id(&pool, "Operator").await;
    let body = json!({ "name": "X", "permissions": [] });

    for (method, path) in [
        ("POST", "/roles".to_owned()),
        ("PUT", format!("/roles/{id}")),
    ] {
        let reply = app.call(method, &path, Some(&body), Some(&cookie)).await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{method} {path}");
    }
}

#[sqlx::test]
async fn saving_a_role_unchanged_writes_no_audit_entry(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let cookie = admin(&app, &pool).await;
    let id = role_id(&pool, "Kontributor Web").await;

    let reply = app
        .call(
            "PUT",
            &format!("/roles/{id}"),
            Some(&json!({ "name": "Kontributor Web", "permissions": ["content.write", "content.publish"] })),
            Some(&cookie),
        )
        .await;

    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    assert_ne!(last_audit(&pool).await.0, "role.update");
}
