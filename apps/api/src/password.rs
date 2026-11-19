use std::{collections::HashSet, sync::LazyLock};

use anyhow::Context;
use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use serde::Serialize;
use utoipa::ToSchema;

/// Breached passwords of 12–128 characters, lowercased; see `data/README.md`.
static COMMON: LazyLock<HashSet<&str>> = LazyLock::new(|| {
    include_str!("../data/common-passwords.txt")
        .lines()
        .collect()
});

/// Pinned so a crate upgrade cannot silently change them: m=19456 KiB, t=2, p=1.
const PARAMS: Params = match Params::new(19_456, 2, 1, None) {
    Ok(params) => params,
    Err(_) => panic!("invalid Argon2 parameters"),
};

fn argon2() -> Argon2<'static> {
    Argon2::new(Algorithm::Argon2id, Version::V0x13, PARAMS)
}

/// Why a new password is refused.
#[derive(Serialize, ToSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub enum PasswordProblem {
    TooShort,
    TooLong,
    Common,
    ContainsUsername,
    ContainsVillageName,
}

/// NIST SP 800-63B: length and a blocklist, no composition rules.
///
/// # Errors
/// The first rule the password breaks.
pub fn check(
    password: &str,
    username: &str,
    village_name: Option<&str>,
) -> Result<(), PasswordProblem> {
    let length = password.chars().count();
    let lower = password.to_lowercase();
    if length < 12 {
        Err(PasswordProblem::TooShort)
    } else if length > 128 {
        Err(PasswordProblem::TooLong)
    } else if COMMON.contains(password.to_ascii_lowercase().as_str()) {
        Err(PasswordProblem::Common)
    } else if lower.contains(username) {
        Err(PasswordProblem::ContainsUsername)
    } else if village_name.is_some_and(|name| lower.contains(&name.to_lowercase())) {
        Err(PasswordProblem::ContainsVillageName)
    } else {
        Ok(())
    }
}

/// Hashes off the async runtime: Argon2id is deliberately slow.
///
/// # Errors
/// When hashing fails or its task panics.
pub async fn hash(password: String) -> anyhow::Result<String> {
    tokio::task::spawn_blocking(move || {
        argon2()
            .hash_password(password.as_bytes())
            .map(|hash| hash.to_string())
            .map_err(|error| anyhow::anyhow!("cannot hash the password: {error}"))
    })
    .await
    .context("the hashing task failed")?
}

/// The outcome of checking a password at sign-in.
pub enum Verdict {
    Wrong,
    /// Carries a new hash when the stored one used other parameters.
    Right {
        rehashed: Option<String>,
    },
}

/// A hash to verify against when there is none, so timing does not reveal unknown usernames.
static DUMMY: LazyLock<String> = LazyLock::new(|| {
    argon2()
        .hash_password(b"no such user")
        .expect("hashing a constant cannot fail")
        .to_string()
});

/// Verifies off the async runtime; a missing hash is always wrong but costs the same.
///
/// # Errors
/// When the stored hash is malformed or the task panics.
pub async fn verify(password: String, stored: Option<String>) -> anyhow::Result<Verdict> {
    tokio::task::spawn_blocking(move || {
        let known = stored.is_some();
        let stored = stored.unwrap_or_else(|| DUMMY.clone());
        let parsed = PasswordHash::new(&stored)
            .map_err(|error| anyhow::anyhow!("malformed password hash: {error}"))?;
        if argon2()
            .verify_password(password.as_bytes(), &parsed)
            .is_err()
            || !known
        {
            return Ok(Verdict::Wrong);
        }
        let current = parsed.algorithm == Algorithm::Argon2id.ident()
            && Params::try_from(&parsed).is_ok_and(|params| params == PARAMS);
        let rehashed = if current {
            None
        } else {
            Some(
                argon2()
                    .hash_password(password.as_bytes())
                    .map_err(|error| anyhow::anyhow!("cannot hash the password: {error}"))?
                    .to_string(),
            )
        };
        Ok(Verdict::Right { rehashed })
    })
    .await
    .context("the hashing task failed")?
}
