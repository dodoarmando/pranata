mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

async fn get(pool: PgPool, uri: &str, request_id: Option<&str>) -> axum::response::Response {
    let mut builder = Request::builder().uri(uri);
    if let Some(id) = request_id {
        builder = builder.header("x-request-id", id);
    }
    pranata::app(common::test_state(pool))
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[sqlx::test]
async fn healthz_reports_version(pool: PgPool) {
    let response = get(pool, "/api/healthz", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json,
        json!({"status": "ok", "version": env!("CARGO_PKG_VERSION")})
    );
}

#[sqlx::test]
async fn every_response_has_a_request_id(pool: PgPool) {
    let response = get(pool.clone(), "/api/healthz", None).await;
    let id = response.headers()["x-request-id"].to_str().unwrap();
    assert!(uuid::Uuid::parse_str(id).is_ok(), "not a uuid: {id}");

    let response = get(pool, "/api/healthz", Some("caller-supplied-id")).await;
    assert_eq!(response.headers()["x-request-id"], "caller-supplied-id");
}

#[sqlx::test]
async fn unknown_route_is_problem_json(pool: PgPool) {
    let response = get(pool, "/api/nope", None).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/problem+json"
    );
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["code"], "not_found");
    assert_eq!(json["status"], 404);
}

#[sqlx::test]
async fn security_headers_present(pool: PgPool) {
    let response = get(pool, "/api/healthz", None).await;
    let headers = response.headers();
    assert_eq!(headers["strict-transport-security"], "max-age=31536000");
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(
        headers["referrer-policy"],
        "strict-origin-when-cross-origin"
    );
    assert_eq!(headers["x-frame-options"], "DENY");
}

#[sqlx::test]
async fn not_found_carries_request_id_and_security_headers(pool: PgPool) {
    let response = get(pool, "/api/nope", None).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let headers = response.headers();
    let id = headers["x-request-id"].to_str().unwrap();
    assert!(uuid::Uuid::parse_str(id).is_ok(), "not a uuid: {id}");
    assert_eq!(headers["x-frame-options"], "DENY");
    assert_eq!(headers["strict-transport-security"], "max-age=31536000");
}
