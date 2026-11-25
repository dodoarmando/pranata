//! Sudo Mode (D-31): sensitive actions need a password confirmation made
//! within the last [`SUDO_WINDOW`].

use anyhow::Context;
use sqlx::PgConnection;
use time::{Duration, OffsetDateTime};

use super::session::AuthSession;
use crate::{
    ApiError,
    audit::{self, AuditAction, NewAuditEntry},
};

/// How long a password confirmation keeps Sudo Mode open.
pub const SUDO_WINDOW: Duration = Duration::minutes(10);

impl AuthSession {
    /// When the Sudo window closes, or `None` when it is closed already or was
    /// never opened.
    #[must_use]
    pub fn sudo_until(&self, now: OffsetDateTime) -> Option<OffsetDateTime> {
        self.reauthenticated_at
            .map(|at| at + SUDO_WINDOW)
            .filter(|until| now < *until)
    }

    /// Check that the session confirmed its password recently enough.
    ///
    /// # Errors
    /// 403 `sudo_required` when the window is closed or was never opened.
    pub fn require_sudo(&self, now: OffsetDateTime) -> Result<(), ApiError> {
        self.sudo_until(now)
            .map(|_| ())
            .ok_or(ApiError::Forbidden("sudo_required"))
    }
}

/// Open the Sudo window of `session` at `now`, after the User proved who they
/// are, and record how (`summary`) in the audit log.
///
/// # Errors
/// Fails when the update or the insert fails.
pub(crate) async fn reopen(
    conn: &mut PgConnection,
    session: &AuthSession,
    now: OffsetDateTime,
    summary: String,
) -> anyhow::Result<()> {
    sqlx::query!(
        "UPDATE sessions SET reauthenticated_at = $2 WHERE token_hash = $1",
        session.token_hash,
        now,
    )
    .execute(&mut *conn)
    .await
    .context("reopening the sudo window")?;
    audit::record(
        conn,
        NewAuditEntry {
            actor: Some(session.user.id),
            action: AuditAction::Reauthenticate,
            object_type: "user",
            object_id: session.user.id.0.to_string(),
            summary,
            changes: Vec::new(),
        },
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        access::RoleRef,
        auth::CurrentUser,
        ids::{RoleId, UserId},
    };

    fn session(reauthenticated_at: Option<OffsetDateTime>) -> AuthSession {
        AuthSession {
            user: CurrentUser {
                id: UserId(uuid::Uuid::nil()),
                username: "budi".into(),
                display_name: "Budi".into(),
                role: RoleRef {
                    id: RoleId(uuid::Uuid::nil()),
                    name: "Administrator".into(),
                    is_administrator: true,
                },
                permissions: Vec::new(),
            },
            csrf_token: String::new(),
            token_hash: Vec::new(),
            reauthenticated_at,
        }
    }

    fn code(result: &Result<(), ApiError>) -> Option<&'static str> {
        match *result {
            Err(ApiError::Forbidden(code)) => Some(code),
            _ => None,
        }
    }

    #[test]
    fn sudo_expires_exactly_at_ten_minutes() {
        let t = OffsetDateTime::from_unix_timestamp(1_795_593_600).unwrap();
        let session = session(Some(t));
        assert!(
            session
                .require_sudo(t + Duration::minutes(9) + Duration::seconds(59))
                .is_ok()
        );
        assert_eq!(
            code(&session.require_sudo(t + Duration::minutes(10))),
            Some("sudo_required")
        );
    }

    #[test]
    fn never_reauthenticated_needs_sudo() {
        let t = OffsetDateTime::from_unix_timestamp(1_795_593_600).unwrap();
        assert_eq!(code(&session(None).require_sudo(t)), Some("sudo_required"));
    }
}
