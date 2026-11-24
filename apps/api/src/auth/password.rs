//! Password hashing (argon2id with the crate's default parameters) and the
//! password length rule.

use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use argon2::{Argon2, PasswordHasher, PasswordVerifier, password_hash::phc::PasswordHash};

use tokio::sync::Semaphore;

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

/// Bounds how many argon2 runs happen at once. Each run needs about 19 MiB,
/// and tokio's blocking pool would otherwise let unauthenticated requests
/// start hundreds of them. It also owns the dummy hash that gives sign-in
/// attempts for unknown users the same cost as real ones.
#[derive(Clone)]
pub struct HashLimiter(Arc<LimiterInner>);

struct LimiterInner {
    permits: Arc<Semaphore>,
    dummy_hash: String,
}

impl HashLimiter {
    /// Allow `permits` concurrent runs (at least one) and prepare the dummy
    /// hash. Call it at startup: failing here is fatal, and there is no cheap
    /// fallback path.
    ///
    /// # Errors
    /// Fails when the dummy hash cannot be computed.
    pub fn new(permits: usize) -> Result<Self> {
        Ok(Self(Arc::new(LimiterInner {
            permits: Arc::new(Semaphore::new(permits.max(1))),
            dummy_hash: hash_password("dummy password for constant sign-in cost")
                .context("preparing the dummy password hash")?,
        })))
    }

    /// One permit per available CPU.
    ///
    /// # Errors
    /// Fails when the dummy hash cannot be computed.
    pub fn for_host() -> Result<Self> {
        Self::new(std::thread::available_parallelism().map_or(1, usize::from))
    }

    /// Run CPU-heavy `work` on a blocking thread once a permit is free. The
    /// permit is held until the work ends, even if the caller goes away.
    ///
    /// # Errors
    /// Fails when the limiter is closed or the task panics.
    pub async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T> {
        let permit = self
            .0
            .permits
            .clone()
            .acquire_owned()
            .await
            .context("hash limiter closed")?;
        tokio::task::spawn_blocking(move || {
            let result = work();
            drop(permit);
            result
        })
        .await
        .context("password hashing task")
    }

    /// Hash a password.
    ///
    /// # Errors
    /// Fails when hashing or the task fails.
    pub async fn hash(&self, password: String) -> Result<String> {
        self.run(move || hash_password(&password)).await?
    }

    /// Verify a password. With `None` (no usable stored hash) it verifies
    /// against the dummy hash, so the cost is the same, and returns false.
    ///
    /// # Errors
    /// Fails when the task fails.
    pub async fn verify(&self, password: String, hash: Option<String>) -> Result<bool> {
        let dummy = self.0.dummy_hash.clone();
        self.run(move || {
            let verified = verify_password(&password, hash.as_deref().unwrap_or(&dummy));
            hash.is_some() && verified
        })
        .await
    }
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
    use std::sync::atomic::{AtomicUsize, Ordering};

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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn limiter_bounds_concurrent_runs() {
        let limiter = HashLimiter::new(2).unwrap();
        let running = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let tasks: Vec<_> = (0..8)
            .map(|_| {
                let (limiter, running, peak) = (limiter.clone(), running.clone(), peak.clone());
                tokio::spawn(async move {
                    limiter
                        .run(move || {
                            let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                            peak.fetch_max(now, Ordering::SeqCst);
                            std::thread::sleep(std::time::Duration::from_millis(30));
                            running.fetch_sub(1, Ordering::SeqCst);
                        })
                        .await
                        .unwrap();
                })
            })
            .collect();
        for task in tasks {
            task.await.unwrap();
        }
        assert!(peak.load(Ordering::SeqCst) <= 2, "more than 2 ran at once");
        assert_eq!(running.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn limiter_always_has_a_permit_and_a_working_dummy() {
        let limiter = HashLimiter::new(0).unwrap();
        let hash = limiter
            .hash("a long enough password".to_owned())
            .await
            .unwrap();
        assert!(
            limiter
                .verify("a long enough password".to_owned(), Some(hash))
                .await
                .unwrap()
        );
        // No stored hash: spends a real verification and never succeeds.
        assert!(
            !limiter
                .verify("a long enough password".to_owned(), None)
                .await
                .unwrap()
        );
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
