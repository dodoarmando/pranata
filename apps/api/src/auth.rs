use std::fmt::Write;

use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::audit::{self, Actor};

/// A random 256-bit token as 64 hex digits, and the SHA-256 hash that is stored instead.
fn new_token() -> (String, Vec<u8>) {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the OS random number generator is unavailable");
    let token = bytes.iter().fold(String::with_capacity(64), |mut hex, b| {
        let _ = write!(hex, "{b:02x}");
        hex
    });
    let hash = token_hash(&token);
    (token, hash)
}

fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token).to_vec()
}

/// Issues a single-use Access Link valid for 24 hours, voiding the User's previous one.
///
/// # Errors
/// When the database write fails.
pub async fn create_access_link(
    conn: &mut PgConnection,
    actor: Actor,
    user_id: Uuid,
) -> sqlx::Result<String> {
    let (token, hash) = new_token();
    sqlx::query!(
        "INSERT INTO access_links (token_hash, user_id, expires_at)
         VALUES ($1, $2, now() + interval '24 hours')
         ON CONFLICT (user_id) DO UPDATE SET token_hash = EXCLUDED.token_hash, expires_at = EXCLUDED.expires_at",
        hash,
        user_id,
    )
    .execute(&mut *conn)
    .await?;
    audit::record(
        conn,
        actor,
        "access_link.create",
        ("user", Some(&user_id.to_string())),
        json!({}),
    )
    .await?;
    Ok(token)
}
