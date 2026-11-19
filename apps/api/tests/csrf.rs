mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::{ORIGIN, TestApp};
use sqlx::PgPool;

const SIGN_IN: &str = r#"{"username":"nobody","password":"wrong password","remember":false}"#;

fn sign_in(headers: &[(&str, &str)]) -> Request<Body> {
    let mut request = Request::post("/auth/sign-in").header("content-type", "application/json");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    request.body(Body::from(SIGN_IN)).unwrap()
}

#[sqlx::test]
async fn unsafe_requests_need_fetch_metadata_or_an_exact_origin(pool: PgPool) {
    let app = TestApp::new(pool);

    for headers in [
        &[("sec-fetch-site", "same-origin")][..],
        &[("origin", ORIGIN)],
    ] {
        // Through the check: the wrong password is what fails.
        assert_eq!(
            app.send(sign_in(headers)).await,
            StatusCode::UNAUTHORIZED,
            "{headers:?}"
        );
    }
    for headers in [
        &[("sec-fetch-site", "cross-site")][..],
        &[("sec-fetch-site", "same-site")],
        &[("sec-fetch-site", "none")],
        // Fetch Metadata wins over a matching Origin.
        &[("sec-fetch-site", "cross-site"), ("origin", ORIGIN)],
        &[("origin", "https://evil.example")],
        &[("origin", "http://desa.example")],
        &[("origin", "null")],
        &[],
    ] {
        assert_eq!(
            app.send(sign_in(headers)).await,
            StatusCode::FORBIDDEN,
            "{headers:?}"
        );
    }
}

#[sqlx::test]
async fn safe_methods_are_not_checked(pool: PgPool) {
    let app = TestApp::new(pool);
    let request = Request::get("/healthz")
        .header("sec-fetch-site", "cross-site")
        .body(Body::empty())
        .unwrap();

    assert_eq!(app.send(request).await, StatusCode::OK);
}

#[sqlx::test]
async fn bodies_are_json_only(pool: PgPool) {
    let app = TestApp::new(pool);

    for content_type in ["text/plain", "application/x-www-form-urlencoded"] {
        let request = Request::post("/auth/sign-in")
            .header("sec-fetch-site", "same-origin")
            .header("content-type", content_type)
            .body(Body::from(SIGN_IN))
            .unwrap();
        assert_eq!(
            app.send(request).await,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "{content_type}"
        );
    }
}
