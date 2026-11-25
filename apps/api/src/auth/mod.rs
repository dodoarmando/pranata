//! Authentication: passwords, server-side sessions, CSRF and Access Links
//! (D-30, D-32). What a User may do comes from their Role (D-33, `crate::access`).

pub mod access_link;
pub mod csrf;
pub mod passkey;
pub mod password;
pub mod session;
pub mod sudo;
pub mod throttle;

use anyhow::Context;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use time::OffsetDateTime;
use utoipa::ToSchema;

use crate::{
    ApiError, FieldError,
    access::{Permission, RoleRef},
    audit::{self, AuditAction, NewAuditEntry},
    ids::{RoleId, UserId},
};

/// The signed-in User, as an axum extractor. A missing or expired session
/// answers 401 `unauthenticated`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CurrentUser {
    #[schema(value_type = uuid::Uuid)]
    pub id: UserId,
    pub username: String,
    pub display_name: String,
    pub role: RoleRef,
    /// What the Role grants, in catalogue order.
    pub permissions: Vec<Permission>,
}

impl CurrentUser {
    /// Check one Permission.
    ///
    /// # Errors
    /// 403 `permission_denied` when the User's Role does not grant it.
    pub fn require(&self, permission: Permission) -> Result<(), ApiError> {
        if self.permissions.contains(&permission) {
            Ok(())
        } else {
            Err(ApiError::Forbidden("permission_denied"))
        }
    }

    /// Check that the User holds at least one of `permissions`.
    ///
    /// # Errors
    /// 403 `permission_denied` when the User's Role grants none of them.
    pub fn require_any(&self, permissions: &[Permission]) -> Result<(), ApiError> {
        if permissions
            .iter()
            .any(|permission| self.permissions.contains(permission))
        {
            Ok(())
        } else {
            Err(ApiError::Forbidden("permission_denied"))
        }
    }
}

/// A User whose credential (password or Passkey) was just verified.
#[derive(Debug, Clone)]
pub struct Identified {
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
pub(crate) fn normalise_username(raw: &str) -> Result<String, ApiError> {
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

/// Create a User with `role` and no password, plus a 24 hour Access Link for
/// them, and record the creation in the audit log, all in one transaction.
/// This is what `pranata user create-admin` runs (with the Administrator Role).
///
/// # Errors
/// `Validation` for a malformed username or blank name, `Conflict`
/// (`username_taken`) when the username exists, `Internal` otherwise.
pub async fn create_user_with_access_link(
    db: &PgPool,
    username: &str,
    display_name: &str,
    role: RoleId,
    created_by: Option<UserId>,
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
        r#"INSERT INTO users (username, display_name, role_id) VALUES ($1, $2, $3)
           ON CONFLICT (username) DO NOTHING
           RETURNING id AS "id: UserId""#,
        username,
        display_name,
        role.0,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Conflict("username_taken"))?;
    let token = access_link::create(&mut tx, id, created_by, now).await?;
    audit::record(
        &mut tx,
        NewAuditEntry {
            actor: created_by,
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
