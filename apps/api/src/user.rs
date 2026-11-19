use anyhow::Context;
use serde_json::json;
use sqlx::PgPool;

use crate::{
    audit::{self, Actor},
    auth,
};

/// Lowercase `[a-z0-9._-]`, 3–32 characters.
fn is_valid_username(username: &str) -> bool {
    (3..=32).contains(&username.len())
        && username
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'))
}

/// Creates a User with the Administrator Role and returns the token of their Access Link.
///
/// # Errors
/// When the username is malformed or already taken, or the database write fails.
pub async fn create_admin(pool: &PgPool, username: &str) -> anyhow::Result<String> {
    anyhow::ensure!(
        is_valid_username(username),
        "a username is 3–32 lowercase letters, digits, dots, underscores or dashes, got {username:?}"
    );
    let mut tx = pool.begin().await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO users (username, role_id) SELECT $1, id FROM roles WHERE administrator
         ON CONFLICT (username) DO NOTHING RETURNING id",
        username,
    )
    .fetch_optional(&mut *tx)
    .await
    .context("cannot save the User")?
    .with_context(|| format!("the username {username:?} is taken"))?;
    audit::record(
        &mut tx,
        Actor::System,
        "user.create",
        ("user", Some(&id.to_string())),
        json!({ "username": username, "role": "Administrator" }),
    )
    .await?;
    let token = auth::create_access_link(&mut tx, Actor::System, id).await?;
    tx.commit().await?;
    Ok(token)
}
