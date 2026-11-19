//! HTTP test harness: drives the real router against a per-test database.
// Each test file compiles this module; not all of them use every helper.
#![allow(dead_code)]

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

pub struct TestApp {
    router: Router,
}

impl TestApp {
    pub fn new(pool: PgPool) -> Self {
        Self {
            router: pranata::app(pool),
        }
    }

    pub async fn get(&self, path: &str) -> (StatusCode, Value) {
        let request = Request::get(path).body(Body::empty()).unwrap();
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    pub async fn post(&self, path: &str, body: &Value) -> (StatusCode, Value) {
        let request = Request::post(path)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    pub async fn request(&self, method: &str, path: &str) -> StatusCode {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .body(Body::empty())
            .unwrap();
        self.router.clone().oneshot(request).await.unwrap().status()
    }
}
