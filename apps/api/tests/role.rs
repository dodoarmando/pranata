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
