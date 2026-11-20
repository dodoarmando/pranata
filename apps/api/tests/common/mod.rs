//! HTTP test harness: drives the real router against a per-test database.
// Each test file compiles this module; not all of them use every helper.
#![allow(dead_code)]

pub mod authenticator;

use std::io::Cursor;

use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use image::{DynamicImage, ImageFormat, RgbaImage};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

use authenticator::Authenticator;

/// A solid `width`×`height` PNG.
pub fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        width,
        height,
        image::Rgba([0, 90, 60, 255]),
    ))
    .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
    .unwrap();
    bytes
}

pub const PASSWORD: &str = "kopi tubruk pagi hari";
pub const ORIGIN: &str = "https://desa.example";

pub struct TestApp {
    router: Router,
    /// The files volume, removed with the app.
    pub files: tempfile::TempDir,
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
        Self::at(pool, None)
    }

    /// An app whose clock stands still at `now`, or runs when `None`.
    pub fn at(pool: PgPool, now: Option<chrono::DateTime<chrono::Utc>>) -> Self {
        let files = tempfile::tempdir().unwrap();
        Self {
            router: pranata::app_at(pool, ORIGIN.to_owned(), files.path().to_owned(), now).unwrap(),
            files,
        }
    }

    /// Sends a request as the browser does through the web app: same-origin, with an optional
    /// JSON body and `name=value` cookie.
    pub async fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
        cookie: Option<&str>,
    ) -> Reply {
        self.call_with(method, path, body, cookie, &[]).await
    }

    /// [`Self::call`] with extra request headers.
    pub async fn call_with(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
        cookie: Option<&str>,
        headers: &[(&str, &str)],
    ) -> Reply {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("sec-fetch-site", "same-origin");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
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

    /// Sends `bytes` as a same-origin upload; the reply of storing it.
    pub async fn upload(&self, path: &str, bytes: Vec<u8>, cookie: &str) -> Reply {
        let request = Request::post(path)
            .header("sec-fetch-site", "same-origin")
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header(header::COOKIE, cookie)
            .body(Body::from(bytes))
            .unwrap();
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        Reply {
            status,
            headers,
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        }
    }

    /// A GET whose body is not JSON, such as an image: status, headers and bytes.
    pub async fn get_raw(&self, path: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
        let request = Request::get(path).body(Body::empty()).unwrap();
        let response = self.router.clone().oneshot(request).await.unwrap();
        let (status, headers) = (response.status(), response.headers().clone());
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, headers, bytes.to_vec())
    }

    pub async fn send(&self, request: Request<Body>) -> StatusCode {
        self.router.clone().oneshot(request).await.unwrap().status()
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
    pub async fn administrator(&self, pool: &PgPool, username: &str) {
        let token = pranata::user::create_administrator(pool, username)
            .await
            .unwrap();
        let (status, _) = self
            .post(
                "/auth/access-link",
                &json!({ "token": token, "password": PASSWORD }),
            )
            .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    /// Creates a User in a fresh Role holding only `permissions`, and signs them in.
    pub async fn user_with(&self, pool: &PgPool, username: &str, permissions: &[&str]) -> String {
        self.administrator(pool, username).await;
        sqlx::query(
            "WITH role AS (INSERT INTO roles (name, permissions) VALUES ($1, $2) RETURNING id)
             UPDATE users SET role_id = role.id FROM role WHERE username = $1",
        )
        .bind(username)
        .bind(permissions)
        .execute(pool)
        .await
        .unwrap();
        self.sign_in(username, false).await
    }

    /// Signs in and returns the session cookie as `name=value`.
    pub async fn sign_in(&self, username: &str, remember: bool) -> String {
        self.sign_in_from(username, remember, &[]).await
    }

    /// [`Self::sign_in`] with extra request headers, such as the forwarded browser and address.
    pub async fn sign_in_from(
        &self,
        username: &str,
        remember: bool,
        headers: &[(&str, &str)],
    ) -> String {
        let reply = self
            .call_with(
                "POST",
                "/auth/sign-in",
                Some(&json!({ "username": username, "password": PASSWORD, "remember": remember })),
                None,
                headers,
            )
            .await;
        assert_eq!(reply.status, StatusCode::NO_CONTENT);
        reply.set_cookie().split(';').next().unwrap().to_owned()
    }

    /// Registers `passkey` under `name` for the signed-in User; the reply of finishing.
    pub async fn add_passkey(&self, cookie: &str, name: &str, passkey: &Authenticator) -> Reply {
        let challenge = self
            .call("POST", "/auth/passkeys/registration", None, Some(cookie))
            .await;
        assert_eq!(challenge.status, StatusCode::OK, "{:?}", challenge.body);
        let body = json!({
            "ceremonyId": challenge.body["ceremonyId"],
            "name": name,
            "credential": passkey.register(&challenge.body["options"]),
        });
        self.call("POST", "/auth/passkeys", Some(&body), Some(cookie))
            .await
    }
}
