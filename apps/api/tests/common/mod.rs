//! HTTP test harness: drives the real router against a per-test database.
// Each test file compiles this module; not all of them use every helper.
#![allow(dead_code)]

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

pub const PASSWORD: &str = "kopi tubruk pagi hari";

pub struct TestApp {
    router: Router,
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Value,
}

impl Reply {
    pub fn set_cookie(&self) -> &str {
        self.headers[header::SET_COOKIE].to_str().unwrap()
    }
}

impl TestApp {
    pub fn new(pool: PgPool) -> Self {
        Self {
            router: pranata::app(pool),
        }
    }

    /// Sends a request with an optional JSON body and `name=value` cookie.
    pub async fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
        cookie: Option<&str>,
    ) -> Reply {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        };
        let response = self.router.clone().oneshot(request.unwrap()).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        Reply {
            status,
            headers,
            body,
        }
    }

    pub async fn get(&self, path: &str) -> (StatusCode, Value) {
        let reply = self.call("GET", path, None, None).await;
        (reply.status, reply.body)
    }

    pub async fn post(&self, path: &str, body: &Value) -> (StatusCode, Value) {
        let reply = self.call("POST", path, Some(body), None).await;
        (reply.status, reply.body)
    }

    pub async fn request(&self, method: &str, path: &str) -> StatusCode {
        self.call(method, path, None, None).await.status
    }

    /// Creates an Administrator from the CLI and sets their password through the Access Link.
    pub async fn admin(&self, pool: &PgPool, username: &str) {
        let token = pranata::user::create_admin(pool, username).await.unwrap();
        let (status, _) = self
            .post(
                "/auth/access-link",
                &json!({ "token": token, "password": PASSWORD }),
            )
            .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    /// Signs in and returns the session cookie as `name=value`.
    pub async fn sign_in(&self, username: &str, remember: bool) -> String {
        let reply = self
            .call(
                "POST",
                "/auth/sign-in",
                Some(&json!({ "username": username, "password": PASSWORD, "remember": remember })),
                None,
            )
            .await;
        assert_eq!(reply.status, StatusCode::NO_CONTENT);
        reply.set_cookie().split(';').next().unwrap().to_owned()
    }
}
