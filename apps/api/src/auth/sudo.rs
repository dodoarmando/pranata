//! Sudo Mode (D-31): sensitive actions need a password confirmation made
//! within the last [`SUDO_WINDOW`].

use time::{Duration, OffsetDateTime};

use super::session::AuthSession;
use crate::ApiError;

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
