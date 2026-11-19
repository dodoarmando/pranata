//! HTTP test harness: drives the real router against a per-test database.

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
}
