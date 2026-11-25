//! Access Links: single-use, short-lived links an Administrator hands over so a
//! User can set a password (D-32, ADR 0007). No email is sent.

use anyhow::Context;
use sqlx::{PgConnection, PgPool};
use time::{Duration, OffsetDateTime};

use super::{
    hash_token,
    password::{self, HashLimiter},
    random_token,
};
use crate::{
    ApiError,
    audit::{self, AuditAction, NewAuditEntry},
    ids::UserId,
};

/// How long a new Access Link stays valid.
pub const LIFETIME: Duration = Duration::hours(24);

/// Revoke the unused Access Links of `user` and return how many were revoked.
/// A User has at most one live link, and deactivation revokes the last one.
///
/// # Errors
/// Fails when the update fails.
pub async fn revoke_unused(
    conn: &mut PgConnection,
    user: UserId,
    now: OffsetDateTime,
) -> anyhow::Result<u64> {
    let result = sqlx::query!(
        "UPDATE access_links SET revoked_at = $2
         WHERE user_id = $1 AND used_at IS NULL AND revoked_at IS NULL",
        user.0,
        now,
    )
    .execute(conn)
    .await
    .context("revoking access links")?;
    Ok(result.rows_affected())
}

/// Create an Access Link for `user` and return its one-time token. Only the
/// token's SHA-256 is stored. The User's earlier unused links are revoked.
///
/// # Errors
/// Fails when the update, the insert or the random source fails.
pub async fn create(
    conn: &mut PgConnection,
    user: UserId,
    created_by: Option<UserId>,
    now: OffsetDateTime,
) -> anyhow::Result<String> {
    let token = random_token()?;
    revoke_unused(&mut *conn, user, now).await?;
    sqlx::query!(
        "INSERT INTO access_links (token_hash, user_id, created_by, created_at, expires_at)
         VALUES ($1, $2, $3, $4, $5)",
        hash_token(&token),
        user.0,
        created_by.map(|creator| creator.0),
        now,
        now + LIFETIME,
    )
    .execute(conn)
    .await
    .context("inserting access link")?;
    Ok(token)
}

struct Link {
    user_id: UserId,
    expires_at: OffsetDateTime,
    used: bool,
    revoked: bool,
    owner_active: bool,
}

fn check(link: Option<Link>, now: OffsetDateTime) -> Result<Link, ApiError> {
    let link = link.ok_or(ApiError::NotFound("access_link_not_found"))?;
    if link.used {
        return Err(ApiError::Gone("access_link_used"));
    }
    if link.revoked {
        return Err(ApiError::Gone("access_link_revoked"));
    }
    if link.expires_at <= now {
        return Err(ApiError::Gone("access_link_expired"));
    }
    if !link.owner_active {
        return Err(ApiError::Gone("access_link_revoked"));
    }
    Ok(link)
}

/// Set the password of the link's User and mark the link used. The User's
/// other sessions end, so a reset also revokes a stolen session.
///
/// # Errors
/// 404 `access_link_not_found`, 410 `access_link_used` or
/// `access_link_revoked` or `access_link_expired`, 422 `password_length`.
pub async fn redeem(
    db: &PgPool,
    hasher: &HashLimiter,
    token: &str,
    new_password: &str,
    now: OffsetDateTime,
) -> Result<(), ApiError> {
    let token_hash = hash_token(token);

    // Cheap checks first, so an invalid link never costs an argon2 run.
    let link = sqlx::query_as!(
        Link,
        r#"SELECT l.user_id AS "user_id: UserId", l.expires_at,
                  l.used_at IS NOT NULL AS "used!",
                  l.revoked_at IS NOT NULL AS "revoked!",
                  u.is_active AS "owner_active!"
           FROM access_links l JOIN users u ON u.id = l.user_id
           WHERE l.token_hash = $1"#,
        token_hash,
    )
    .fetch_optional(db)
    .await?;
    check(link, now)?;
    password::validate_length(new_password)?;

    let hash = hasher.hash(new_password.to_owned()).await?;

    let mut tx = db.begin().await?;
    // Lock the row and re-check: two requests with one link must not both win.
    let link = sqlx::query_as!(
        Link,
        r#"SELECT l.user_id AS "user_id: UserId", l.expires_at,
                  l.used_at IS NOT NULL AS "used!",
                  l.revoked_at IS NOT NULL AS "revoked!",
                  u.is_active AS "owner_active!"
           FROM access_links l JOIN users u ON u.id = l.user_id
           WHERE l.token_hash = $1 FOR UPDATE"#,
        token_hash,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let link = check(link, now)?;

    sqlx::query!(
        "UPDATE users SET password_hash = $2 WHERE id = $1",
        link.user_id.0,
        hash,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE access_links SET used_at = $2 WHERE token_hash = $1",
        token_hash,
        now,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM sessions WHERE user_id = $1", link.user_id.0)
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut tx,
        NewAuditEntry {
            actor: Some(link.user_id),
            action: AuditAction::Update,
            object_type: "user",
            object_id: link.user_id.0.to_string(),
            summary: "Set a password with an Access Link".to_owned(),
            changes: Vec::new(),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
