mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::json;
use sqlx::PgPool;

const GOOD_PASSWORD: &str = "kopi tubruk pagi hari";

#[sqlx::test]
async fn an_access_link_sets_the_password_only_once(pool: PgPool) {
    let token = pranata::user::create_admin(&pool, "admin").await.unwrap();
    let app = TestApp::new(pool);
    let set = json!({ "token": token, "password": GOOD_PASSWORD });

    let (first, _) = app.post("/auth/access-link", &set).await;
    let (second, _) = app.post("/auth/access-link", &set).await;

    assert_eq!(first, StatusCode::NO_CONTENT);
    assert_eq!(second, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn an_unknown_or_expired_access_link_is_refused(pool: PgPool) {
    let token = pranata::user::create_admin(&pool, "admin").await.unwrap();
    sqlx::query("UPDATE access_links SET expires_at = now() - interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();
    let app = TestApp::new(pool);

    for token in [token, "0".repeat(64)] {
        let (status, _) = app
            .post(
                "/auth/access-link",
                &json!({ "token": token, "password": GOOD_PASSWORD }),
            )
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[sqlx::test]
async fn a_password_is_12_to_128_characters_and_neither_common_nor_personal(pool: PgPool) {
    pranata::village::set_code(&pool, "1101012001")
        .await
        .unwrap();
    let village = pranata::village::region_name("11.01.01.2001").unwrap();
    let token = pranata::user::create_admin(&pool, "kaur.umum")
        .await
        .unwrap();
    let app = TestApp::new(pool);

    for (password, problem) in [
        ("a".repeat(11), "tooShort"),
        ("a".repeat(129), "tooLong"),
        ("QWEasdzxc123".to_owned(), "common"),
        ("saya KAUR.UMUM desa".to_owned(), "containsUsername"),
        (
            format!("kantor {} jaya", village.to_uppercase()),
            "containsVillageName",
        ),
    ] {
        let (status, body) = app
            .post(
                "/auth/access-link",
                &json!({ "token": token, "password": password }),
            )
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{password}");
        assert_eq!(body, json!({ "problem": problem }), "{password}");
    }
    // Characters, not bytes: twelve two-byte letters pass.
    let (status, _) = app
        .post(
            "/auth/access-link",
            &json!({ "token": token, "password": "éèêëāăąçćĉċč" }),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}
