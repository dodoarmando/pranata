//! Password hashing (argon2id with the crate's default parameters) and the
//! password length rule.

use std::sync::OnceLock;

use anyhow::{Result, anyhow};
use argon2::{Argon2, PasswordHasher, PasswordVerifier, password_hash::phc::PasswordHash};

use crate::{ApiError, FieldError};

/// Shortest accepted password, in characters.
pub const MIN_LENGTH: usize = 10;
/// Longest accepted password, in characters.
pub const MAX_LENGTH: usize = 128;

/// Hash a password into a PHC string.
///
/// # Errors
/// Fails when the hasher or the system random source fails.
pub fn hash_password(password: &str) -> Result<String> {
    let hash = Argon2::default()
        .hash_password(password.as_bytes())
        .map_err(|error| anyhow!("hashing password: {error}"))?;
    Ok(hash.to_string())
}

/// Check a password against a PHC string. A malformed hash never verifies.
#[must_use]
pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(hash) = hash.parse::<PasswordHash>() else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &hash)
        .is_ok()
}

/// Spend the time of one verification without a real hash. Sign-in calls this
/// for unknown, inactive and password-less accounts, so the response time does
/// not reveal whether the username exists.
pub fn verify_against_dummy(password: &str) {
    static DUMMY: OnceLock<String> = OnceLock::new();
    let hash = DUMMY.get_or_init(|| {
        // Hashing cannot realistically fail; an empty string would only make
        // the dummy check cheap, never wrongly succeed.
        hash_password("dummy password for constant sign-in cost").unwrap_or_default()
    });
    let _ = verify_password(password, hash);
}

/// Enforce the length rule: 10 to 128 characters, no composition rules.
///
/// # Errors
/// `Validation` with code `password_length` on the `password` field.
pub fn validate_length(password: &str) -> Result<(), ApiError> {
    if (MIN_LENGTH..=MAX_LENGTH).contains(&password.chars().count()) {
        return Ok(());
    }
    Err(ApiError::Validation(vec![FieldError {
        field: "password".to_owned(),
        code: "password_length".to_owned(),
    }]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_verifies_and_is_salted() {
        let a = hash_password("correct horse battery").unwrap();
        let b = hash_password("correct horse battery").unwrap();
        assert_ne!(a, b, "each hash needs its own salt");
        assert!(a.starts_with("$argon2id$"));
        assert!(verify_password("correct horse battery", &a));
        assert!(!verify_password("wrong horse battery", &a));
    }

    #[test]
    fn malformed_hash_never_verifies() {
        assert!(!verify_password("anything at all", "not-a-hash"));
        assert!(!verify_password("anything at all", ""));
    }

    #[test]
    fn length_counts_characters_not_bytes() {
        assert!(validate_length(&"a".repeat(9)).is_err());
        assert!(validate_length(&"a".repeat(10)).is_ok());
        assert!(validate_length(&"a".repeat(128)).is_ok());
        assert!(validate_length(&"a".repeat(129)).is_err());
        // 10 multi-byte characters are long enough.
        assert!(validate_length(&"é".repeat(10)).is_ok());
    }

    #[test]
    fn length_error_names_the_field_and_code() {
        match validate_length("short") {
            Err(ApiError::Validation(errors)) => {
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].field, "password");
                assert_eq!(errors[0].code, "password_length");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
}
