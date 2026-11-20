//! The public-form module: what keeps machines off the forms residents send without an
//! account (Complaints now, Letter Requests in R3), with no third-party captcha.
//!
//! - A honeypot field people never see and bots fill.
//! - An ALTCHA-style proof of work: the browser finds the number behind a signed SHA-256
//!   challenge, a second or two for one resident and a real cost for a bot sending thousands.
//! - At most 5 sendings an hour from one address.
//!
//! Whatever stops a sending, the sender gets the answer it would have had had it gone through.

use std::{
    collections::HashMap,
    fmt::Write,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{Json, extract::State};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::ToSchema;

use crate::{Clock, throttle::RateLimit};

/// The largest secret number: about 25,000 hashes on average, within 2 s on a lab machine and
/// 5 s on the reference phone.
const MAX_NUMBER: u32 = 50_000;
/// How long a challenge stays solvable: the browser solves it as the form opens, and a resident
/// may take a while to write.
const LIFETIME: TimeDelta = TimeDelta::hours(2);
const SENDINGS_PER_HOUR: usize = 5;
const LOOKUPS_PER_MINUTE: usize = 20;

/// The signing key, the proofs already spent, and the sendings and status lookups per address.
// shortcut: in memory, for the single API process (ADR-0001); a restart voids open challenges,
// so a form left open across a deploy is answered as if sent but stored nowhere.
pub(crate) struct PublicForm {
    key: [u8; 32],
    /// Each spent challenge until it expires, so a proof is used once.
    spent: Mutex<HashMap<String, DateTime<Utc>>>,
    sendings: RateLimit,
    lookups: RateLimit,
}

/// A proof-of-work puzzle: find `number` in `0..=maxNumber` whose SHA-256 of `salt` followed
/// by its decimal digits, in hex, is `challenge`.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[expect(clippy::struct_field_names, reason = "the names of ALTCHA's challenge")]
pub struct Challenge {
    /// Always `SHA-256`.
    algorithm: &'static str,
    challenge: String,
    max_number: u32,
    salt: String,
    signature: String,
}

/// A solved [`Challenge`], sent with the form.
#[derive(Deserialize, ToSchema)]
pub struct Proof {
    salt: String,
    number: u32,
    signature: String,
}

impl Default for PublicForm {
    fn default() -> Self {
        let mut key = [0; 32];
        getrandom::fill(&mut key).expect("the OS random number generator is unavailable");
        Self::with_key(key)
    }
}

impl PublicForm {
    fn with_key(key: [u8; 32]) -> Self {
        Self {
            key,
            spent: Mutex::default(),
            sendings: RateLimit::new(SENDINGS_PER_HOUR, Duration::from_hours(1)),
            lookups: RateLimit::new(LOOKUPS_PER_MINUTE, Duration::from_mins(1)),
        }
    }

    fn challenge(&self, now: DateTime<Utc>) -> Challenge {
        let mut nonce = [0; 12];
        getrandom::fill(&mut nonce).expect("the OS random number generator is unavailable");
        let mut number = [0; 4];
        getrandom::fill(&mut number).expect("the OS random number generator is unavailable");
        let number = u32::from_le_bytes(number) % (MAX_NUMBER + 1);
        let salt = format!("{}?expires={}", hex(&nonce), (now + LIFETIME).timestamp());
        let challenge = hex(&Sha256::digest(format!("{salt}{number}")));
        Challenge {
            algorithm: "SHA-256",
            signature: hex(&self.sign(challenge.as_bytes())),
            challenge,
            max_number: MAX_NUMBER,
            salt,
        }
    }

    /// Whether a sending from `ip` gets through: the `honeypot` is empty, the `proof` is
    /// valid, unexpired and unspent, and the address has sent fewer than 5 in the past hour.
    /// A valid proof is spent even when the address is over its limit.
    pub(crate) fn admit(
        &self,
        honeypot: Option<&str>,
        proof: Option<&Proof>,
        ip: Option<IpAddr>,
        now: DateTime<Utc>,
    ) -> bool {
        honeypot.is_none_or(str::is_empty)
            && proof.is_some_and(|proof| self.spend(proof, now))
            && self.sendings.admit(ip)
    }

    /// Whether `ip` may read a status page by its code, at most 20 a minute.
    pub(crate) fn admit_lookup(&self, ip: Option<IpAddr>) -> bool {
        self.lookups.admit(ip)
    }

    fn spend(&self, proof: &Proof, now: DateTime<Utc>) -> bool {
        let challenge = hex(&Sha256::digest(format!("{}{}", proof.salt, proof.number)));
        let expires = proof
            .salt
            .rsplit_once("?expires=")
            .and_then(|(_, at)| DateTime::from_timestamp(at.parse().ok()?, 0));
        let Some(expires) = expires.filter(|expires| now < *expires) else {
            return false;
        };
        if proof.number > MAX_NUMBER || hex(&self.sign(challenge.as_bytes())) != proof.signature {
            return false;
        }
        let mut spent = self.spent.lock().expect("public form lock poisoned");
        spent.retain(|_, expires| now < *expires);
        spent.insert(challenge, expires).is_none()
    }

    /// HMAC-SHA256 (RFC 2104) with the 32-byte key, which fits a block without hashing.
    fn sign(&self, message: &[u8]) -> [u8; 32] {
        let mut block = [0; 64];
        block[..32].copy_from_slice(&self.key);
        let inner = Sha256::new()
            .chain_update(block.map(|byte| byte ^ 0x36))
            .chain_update(message)
            .finalize();
        Sha256::new()
            .chain_update(block.map(|byte| byte ^ 0x5c))
            .chain_update(inner)
            .finalize()
            .into()
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    })
}

/// A fresh proof-of-work challenge for a public form, solved in the browser as the form opens.
#[utoipa::path(
    get,
    path = "/public-form/challenge",
    operation_id = "getPublicFormChallenge",
    tag = "public-form",
    responses((status = OK, description = "A challenge valid for 2 hours", body = Challenge)),
)]
pub(crate) async fn get_challenge(
    State(form): State<Arc<PublicForm>>,
    State(clock): State<Clock>,
) -> Json<Challenge> {
    Json(form.challenge(clock.now()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solve(challenge: &Challenge) -> Proof {
        let number = (0..=challenge.max_number)
            .find(|n| hex(&Sha256::digest(format!("{}{n}", challenge.salt))) == challenge.challenge)
            .expect("the challenge has a solution");
        Proof {
            salt: challenge.salt.clone(),
            number,
            signature: challenge.signature.clone(),
        }
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "as the API reads a client address"
    )]
    fn ip(last: u8) -> Option<IpAddr> {
        Some(IpAddr::from([203, 0, 113, last]))
    }

    #[test]
    fn signs_as_rfc_4231_test_case_1() {
        let mut key = [0; 32];
        key[..20].fill(0x0b);
        assert_eq!(
            hex(&PublicForm::with_key(key).sign(b"Hi There")),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn a_solved_challenge_admits_one_sending() {
        let form = PublicForm::default();
        let now = Utc::now();
        let proof = solve(&form.challenge(now));
        assert!(form.admit(Some(""), Some(&proof), ip(1), now));
        assert!(!form.admit(None, Some(&proof), ip(2), now), "spent");
    }

    #[test]
    fn a_filled_honeypot_or_a_missing_proof_is_refused() {
        let form = PublicForm::default();
        let now = Utc::now();
        let proof = solve(&form.challenge(now));
        assert!(!form.admit(Some("https://spam.example"), Some(&proof), ip(1), now));
        assert!(!form.admit(None, None, ip(1), now));
    }

    #[test]
    fn a_wrong_number_signature_or_salt_is_refused() {
        let form = PublicForm::default();
        let now = Utc::now();
        let challenge = form.challenge(now);
        let proof = solve(&challenge);
        let wrong_number = Proof {
            number: (proof.number + 1) % MAX_NUMBER,
            ..solve(&challenge)
        };
        let later = format!("{}9", proof.salt);
        let longer_life = Proof {
            salt: later,
            ..solve(&challenge)
        };
        let other_key = solve(&PublicForm::default().challenge(now));
        for proof in [wrong_number, longer_life, other_key] {
            assert!(!form.admit(None, Some(&proof), ip(1), now));
        }
        assert!(form.admit(None, Some(&proof), ip(1), now));
    }

    #[test]
    fn a_challenge_expires_after_2_hours() {
        let form = PublicForm::default();
        let now = Utc::now();
        let late = solve(&form.challenge(now));
        assert!(!form.admit(None, Some(&late), ip(1), now + LIFETIME));
        let in_time = solve(&form.challenge(now));
        assert!(form.admit(
            None,
            Some(&in_time),
            ip(1),
            now + LIFETIME - TimeDelta::seconds(1)
        ));
    }

    #[test]
    fn an_address_sends_5_an_hour() {
        let form = PublicForm::default();
        let now = Utc::now();
        for _ in 0..5 {
            assert!(form.admit(None, Some(&solve(&form.challenge(now))), ip(1), now));
        }
        assert!(!form.admit(None, Some(&solve(&form.challenge(now))), ip(1), now));
        assert!(form.admit(None, Some(&solve(&form.challenge(now))), ip(2), now));
    }
}
