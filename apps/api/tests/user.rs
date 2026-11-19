use sqlx::PgPool;

#[sqlx::test]
async fn usernames_are_3_to_32_lowercase_letters_digits_dots_underscores_or_dashes(pool: PgPool) {
    for username in ["abc", "kaur.umum_2-a", &"a".repeat(32)] {
        assert!(
            pranata::user::create_admin(&pool, username).await.is_ok(),
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
            pranata::user::create_admin(&pool, username).await.is_err(),
            "{username} was accepted"
        );
    }
}

#[sqlx::test]
async fn a_username_is_never_reused(pool: PgPool) {
    pranata::user::create_admin(&pool, "admin").await.unwrap();

    let again = pranata::user::create_admin(&pool, "admin").await;

    assert!(again.unwrap_err().to_string().contains("taken"));
}

#[sqlx::test]
async fn each_access_link_is_a_fresh_256_bit_token(pool: PgPool) {
    let first = pranata::user::create_admin(&pool, "admin").await.unwrap();
    let second = pranata::user::create_admin(&pool, "admin2").await.unwrap();

    assert_eq!(first.len(), 64);
    assert!(first.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_ne!(first, second);
}
