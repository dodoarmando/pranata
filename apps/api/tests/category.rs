mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::json;
use sqlx::PgPool;

async fn app_with(pool: &PgPool, permissions: &[&str]) -> (TestApp, String) {
    let app = TestApp::new(pool.clone());
    let cookie = app.user_with(pool, "redaksi", permissions).await;
    (app, cookie)
}

#[sqlx::test]
async fn a_publisher_manages_categories_and_the_public_lists_them_by_name(pool: PgPool) {
    let (app, cookie) = app_with(&pool, &["content.publish", "audit.read"]).await;

    let first = app
        .call(
            "POST",
            "/categories",
            Some(&json!({ "name": " Pembangunan " })),
            Some(&cookie),
        )
        .await;
    let second = app
        .call(
            "POST",
            "/categories",
            Some(&json!({ "name": "kesehatan" })),
            Some(&cookie),
        )
        .await;
    let id = second.body["id"].as_str().unwrap();
    let renamed = app
        .call(
            "PUT",
            &format!("/categories/{id}"),
            Some(&json!({ "name": "Kesehatan" })),
            Some(&cookie),
        )
        .await;

    assert_eq!(first.status, StatusCode::CREATED);
    assert_eq!(renamed.status, StatusCode::NO_CONTENT);
    let (status, list) = app.get("/categories").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        list,
        json!([
            { "id": second.body["id"], "name": "Kesehatan" },
            { "id": first.body["id"], "name": "Pembangunan" },
        ])
    );
    let entries = app
        .call("GET", "/audit-entries", None, Some(&cookie))
        .await
        .body["entries"]
        .clone();
    assert_eq!(entries[0]["action"], "category.update");
    assert_eq!(
        entries[0]["changes"],
        json!([{ "field": "name", "from": "kesehatan", "to": "Kesehatan", "sensitive": false }])
    );

    let deleted = app
        .call("DELETE", &format!("/categories/{id}"), None, Some(&cookie))
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/categories").await.1.as_array().unwrap().len(), 1);
    let again = app
        .call("DELETE", &format!("/categories/{id}"), None, Some(&cookie))
        .await;
    assert_eq!(again.status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn names_are_unique_whatever_the_case_and_never_blank(pool: PgPool) {
    let (app, cookie) = app_with(&pool, &["content.publish"]).await;
    app.call(
        "POST",
        "/categories",
        Some(&json!({ "name": "Pembangunan" })),
        Some(&cookie),
    )
    .await;

    for (name, status) in [
        ("PEMBANGUNAN", StatusCode::CONFLICT),
        ("  ", StatusCode::UNPROCESSABLE_ENTITY),
        (&"x".repeat(51), StatusCode::UNPROCESSABLE_ENTITY),
    ] {
        let reply = app
            .call(
                "POST",
                "/categories",
                Some(&json!({ "name": name })),
                Some(&cookie),
            )
            .await;
        assert_eq!(reply.status, status, "{name}");
    }
}

#[sqlx::test]
async fn only_a_publisher_manages_categories(pool: PgPool) {
    let (app, writer) = app_with(&pool, &["content.write"]).await;

    let reply = app
        .call(
            "POST",
            "/categories",
            Some(&json!({ "name": "Pembangunan" })),
            Some(&writer),
        )
        .await;

    assert_eq!(reply.status, StatusCode::FORBIDDEN);
    let anonymous = app
        .post("/categories", &json!({ "name": "Pembangunan" }))
        .await;
    assert_eq!(anonymous.0, StatusCode::UNAUTHORIZED);
}
