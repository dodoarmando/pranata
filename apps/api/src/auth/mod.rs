//! Authentication: passwords, server-side sessions, CSRF and Access Links
//! (D-30, D-32). In R0 every User has full access; roles arrive with plan R1-A.

pub mod access_link;
pub mod csrf;
pub mod password;
pub mod session;

use anyhow::Context;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::{
    ApiError, FieldError,
    audit::{self, AuditAction, NewAuditEntry},
    ids::UserId,
};

/// The signed-in User, as an axum extractor. A missing or expired session
/// answers 401 `unauthenticated`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CurrentUser {
    #[schema(value_type = uuid::Uuid)]
    pub id: UserId,
    pub username: String,
    pub display_name: String,
}

/// A freshly created User and the Access Link that lets them set a password.
pub struct NewUser {
    pub id: UserId,
    /// The one-time secret; only its hash is stored.
    pub access_link_token: String,
}

impl std::fmt::Debug for NewUser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewUser")
            .field("id", &self.id)
            .field("access_link_token", &"[redacted]")
            .finish()
    }
}

/// 32 random bytes from the operating system, base64url without padding.
///
/// # Errors
/// Fails when the system random source is unavailable.
pub(crate) fn random_token() -> anyhow::Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| anyhow::anyhow!("reading system randomness: {error}"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// SHA-256 of a token: what the database stores instead of the token.
pub(crate) fn hash_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// Lower-case and trim a username, then check it against the stored format.
fn normalise_username(raw: &str) -> Result<String, ApiError> {
    let username = raw.trim().to_lowercase();
    let mut chars = username.chars();
    let valid = (3..=64).contains(&username.len())
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c));
    if valid {
        Ok(username)
    } else {
        Err(ApiError::Validation(vec![FieldError {
            field: "username".to_owned(),
            code: "username_invalid".to_owned(),
        }]))
    }
}

/// Create a User without a password plus a 24 hour Access Link for them, and
/// record the creation in the audit log, all in one transaction. This is what
/// `pranata user create-admin` runs.
///
/// # Errors
/// `Validation` for a malformed username or blank name, `Conflict`
/// (`username_taken`) when the username exists, `Internal` otherwise.
pub async fn create_user_with_access_link(
    db: &PgPool,
    username: &str,
    display_name: &str,
    now: OffsetDateTime,
) -> Result<NewUser, ApiError> {
    let username = normalise_username(username)?;
    let display_name = display_name.trim();
    if display_name.is_empty() {
        return Err(ApiError::Validation(vec![FieldError {
            field: "display_name".to_owned(),
            code: "required".to_owned(),
        }]));
    }

    let mut tx = db.begin().await?;
    let id = sqlx::query_scalar!(
        r#"INSERT INTO users (username, display_name) VALUES ($1, $2)
           ON CONFLICT (username) DO NOTHING
           RETURNING id AS "id: UserId""#,
        username,
        display_name,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Conflict("username_taken"))?;
    let token = access_link::create(&mut tx, id, None, now).await?;
    audit::record(
        &mut tx,
        NewAuditEntry {
            actor: None,
            action: AuditAction::Create,
            object_type: "user",
            object_id: id.0.to_string(),
            summary: format!("Created user {username}"),
            changes: Vec::new(),
        },
    )
    .await
    .context("recording user creation")?;
    tx.commit().await?;
    Ok(NewUser {
        id,
        access_link_token: token,
    })
}
