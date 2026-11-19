use std::{collections::HashSet, sync::LazyLock};

use anyhow::Context;
use argon2::{Algorithm, Argon2, Params, Version, password_hash::PasswordHasher};
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
