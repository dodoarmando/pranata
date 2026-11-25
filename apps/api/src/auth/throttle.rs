//! Sign-in throttling per account (D-34): too many wrong passwords lock the
//! account for a while. The count lives in PostgreSQL, so it survives a
//! restart and is shared by every API process.
//!
//! Unknown usernames are counted exactly like real ones, so a 429 never
//! reveals whether a User exists. Only well-formed usernames are recorded,
//! so junk input cannot grow the table.

use anyhow::Context;
use sqlx::{PgConnection, PgPool};
use time::{Duration, OffsetDateTime};

use super::normalise_username;
use crate::ApiError;

/// How far back failed sign-ins count.
pub const ACCOUNT_WINDOW: Duration = Duration::minutes(15);
/// How many failures in the window lock the account.
pub const ACCOUNT_LIMIT: i64 = 10;
/// Rows older than this are deleted whenever a failure is recorded.
const RETENTION: Duration = Duration::days(1);

/// Refuse the attempt when the account has [`ACCOUNT_LIMIT`] or more failures
/// within [`ACCOUNT_WINDOW`]. Runs before the password check, so a throttled
/// attempt costs no argon2 run.
///
/// # Errors
/// 429 `too_many_attempts` with `Retry-After` set to the seconds until the
/// lock lifts; `Internal` when the query fails.
pub async fn check(db: &PgPool, username: &str, now: OffsetDateTime) -> Result<(), ApiError> {
    let Ok(username) = normalise_username(username) else {
        return Ok(());
    };
    // The newest ACCOUNT_LIMIT failures: when all of them are there, the last
    // one is the failure whose leaving the window lifts the lock.
    let newest = sqlx::query_scalar!(
        r#"SELECT at FROM login_failures
           WHERE username = $1 AND at > $2
           ORDER BY at DESC LIMIT $3"#,
        username,
        now - ACCOUNT_WINDOW,
        ACCOUNT_LIMIT,
    )
    .fetch_all(db)
    .await?;
    if i64::try_from(newest.len()).is_ok_and(|count| count >= ACCOUNT_LIMIT)
        && let Some(anchor) = newest.last()
    {
        let wait = (*anchor + ACCOUNT_WINDOW - now).whole_seconds().max(1);
        return Err(ApiError::TooManyRequests {
            code: "too_many_attempts",
            retry_after: u64::try_from(wait).unwrap_or(1),
        });
    }
    Ok(())
}

/// Count one failed sign-in. Runs on the pool, outside any request
/// transaction, so a rollback cannot lose it. Also deletes rows older than a
/// day. A username that fails the stored format records nothing.
///
/// # Errors
/// Fails when the insert or the cleanup fails.
pub async fn record_failure(
    db: &PgPool,
    username: &str,
    now: OffsetDateTime,
) -> anyhow::Result<()> {
    let Ok(username) = normalise_username(username) else {
        return Ok(());
    };
    sqlx::query!(
        "INSERT INTO login_failures (username, at) VALUES ($1, $2)",
        username,
        now,
    )
    .execute(db)
    .await
    .context("recording a failed sign-in")?;
    sqlx::query!("DELETE FROM login_failures WHERE at < $1", now - RETENTION)
        .execute(db)
        .await
        .context("deleting old failed sign-ins")?;
    Ok(())
}

/// Forget the failures of `username`: the User signed in, or redeemed an
/// Access Link (so an Administrator can unlock an account by issuing one).
///
/// # Errors
/// Fails when the delete fails.
pub async fn clear(conn: &mut PgConnection, username: &str) -> anyhow::Result<()> {
    sqlx::query!("DELETE FROM login_failures WHERE username = $1", username)
        .execute(conn)
        .await
        .context("clearing failed sign-ins")?;
    Ok(())
}
