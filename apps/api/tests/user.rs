mod common;

use sqlx::PgPool;

#[sqlx::test]
async fn usernames_are_3_to_32_lowercase_letters_digits_dots_underscores_or_dashes(pool: PgPool) {
    for username in ["abc", "kaur.umum_2-a", &"a".repeat(32)] {
        assert!(
            pranata::user::create_administrator(&pool, username)
                .await
                .is_ok(),
            "{username} was refused"
        );
    }
    for username in [
        "ab",
        &"a".repeat(33),
        "Admin",
        "kaur umum",
        "kaur@desa",
        "kaür",
    ] {
        assert!(
            pranata::user::create_administrator(&pool, username)
                .await
                .is_err(),
            "{username} was accepted"
        );
    }
}

#[sqlx::test]
async fn a_username_is_never_reused(pool: PgPool) {
    pranata::user::create_administrator(&pool, "admin")
        .await
        .unwrap();

    let again = pranata::user::create_administrator(&pool, "admin").await;

    assert!(again.unwrap_err().to_string().contains("taken"));
}

#[sqlx::test]
async fn each_access_link_is_a_fresh_256_bit_token(pool: PgPool) {
    let first = pranata::user::create_administrator(&pool, "admin")
        .await
        .unwrap();
    let second = pranata::user::create_administrator(&pool, "admin2")
        .await
        .unwrap();

    assert_eq!(first.len(), 64);
    assert!(first.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_ne!(first, second);
}

#[sqlx::test]
async fn the_cli_resets_a_locked_out_administrator_as_the_system(pool: PgPool) {
    let app = common::TestApp::new(pool.clone());
    app.administrator(&pool, "admin").await;
    let session = app.sign_in("admin", false).await;

    let token = pranata::user::reset_access_link(&pool, "admin")
        .await
        .unwrap();

    assert_eq!(token.len(), 64);
    let me = app.call("GET", "/auth/me", None, Some(&session)).await;
    assert_eq!(me.status, axum::http::StatusCode::UNAUTHORIZED);
    let entries: Vec<(String, String)> =
        sqlx::query_as("SELECT action, actor FROM audit_entries ORDER BY id DESC LIMIT 2")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        entries,
        [
            ("access_link.create".to_owned(), "system".to_owned()),
            ("user.sessions_revoke".to_owned(), "system".to_owned()),
        ]
    );
    let (status, _) = app
        .post(
            "/auth/access-link",
            &serde_json::json!({ "token": token, "password": common::PASSWORD }),
        )
        .await;
    assert_eq!(status, axum::http::StatusCode::NO_CONTENT);
}

#[sqlx::test]
async fn the_cli_resets_only_a_known_active_user(pool: PgPool) {
    pranata::user::create_administrator(&pool, "admin")
        .await
        .unwrap();
    pranata::user::create_administrator(&pool, "admin2")
        .await
        .unwrap();
    sqlx::query("UPDATE users SET active = false WHERE username = 'admin2'")
        .execute(&pool)
        .await
        .unwrap();

    let unknown = pranata::user::reset_access_link(&pool, "nobody").await;
    let inactive = pranata::user::reset_access_link(&pool, "admin2").await;

    assert!(unknown.unwrap_err().to_string().contains("no User"));
    assert!(inactive.unwrap_err().to_string().contains("inactive"));
}
