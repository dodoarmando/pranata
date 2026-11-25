//! Passkeys (D-30, D-32): WebAuthn credentials a User signs in with instead of
//! a password.
//!
//! Every ceremony has two steps. The start stores webauthn-rs's state in
//! `webauthn_challenges` (never on the client) for [`CHALLENGE_LIFETIME`] and
//! hands out its id with the options for the browser. The finish takes the
//! state back by that id: the row is deleted in the same statement, and only
//! when it belongs to the same ceremony and the same User and has not
//! expired. Anything else answers like an unknown challenge (410
//! `passkey_challenge_expired`) and leaves the row alone. A challenge is used
//! up by its first matching finish, whether the verification then succeeds or
//! not, so a WebAuthn response can never be replayed.
//!
//! Lock order: a transaction here locks the User before their Passkeys (see
//! [`crate::access::users`]). Every write that adds or removes a Passkey
//! locks the User first, so the last-Administrator rule can count them.

use anyhow::Context;
use serde::{Serialize, de::DeserializeOwned};
use sqlx::{PgConnection, PgPool};
use time::{Duration, OffsetDateTime};
use url::Url;
use utoipa::ToSchema;
use uuid::Uuid;
use webauthn_rs::prelude::{
    AuthenticationResult, CreationChallengeResponse, DiscoverableAuthentication, Passkey,
    PasskeyRegistration, PublicKeyCredential, RegisterPublicKeyCredential, Webauthn,
    WebauthnBuilder,
};
use webauthn_rs_proto::{
    AuthenticatorSelectionCriteria, ResidentKeyRequirement, UserVerificationPolicy,
};

use super::{CurrentUser, Identified};
use crate::{
    ApiError, FieldError,
    access::users::guard_own_passkey_removal,
    audit::{self, AuditAction, NewAuditEntry},
    ids::{PasskeyId, UserId},
};

/// How long a ceremony may take between its start and its finish.
pub const CHALLENGE_LIFETIME: Duration = Duration::minutes(5);
/// The longest Passkey name, in characters.
const NAME_MAX: usize = 50;

/// The Relying Party for `origin` (`PRANATA_PUBLIC_ORIGIN`): its host is the
/// RP ID, the origin itself the only allowed origin.
///
/// # Errors
/// Fails when the origin has no host or webauthn-rs rejects it.
pub fn relying_party(origin: &Url) -> anyhow::Result<Webauthn> {
    let rp_id = origin.host_str().context("the public origin has no host")?;
    WebauthnBuilder::new(rp_id, origin)
        .and_then(|builder| builder.rp_name("Pranata").build())
        .map_err(|error| anyhow::anyhow!("configuring WebAuthn for {origin}: {error}"))
}

/// A Passkey as its owner sees it.
#[derive(Debug, Serialize, ToSchema)]
pub struct PasskeySummary {
    #[schema(value_type = uuid::Uuid)]
    pub id: PasskeyId,
    pub name: String,
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    pub created_at: OffsetDateTime,
    /// The last sign-in with it; null when never used.
    #[serde(with = "time::serde::rfc3339::option")]
    #[schema(value_type = Option<String>, format = DateTime)]
    pub last_used_at: Option<OffsetDateTime>,
}

/// The start of a ceremony: what to send back with the finish, and the
/// options to hand to the browser's WebAuthn API.
#[derive(Debug, Serialize, ToSchema)]
pub struct PasskeyChallenge {
    /// Valid for five minutes, for one finish.
    pub challenge_id: Uuid,
    /// `CredentialCreationOptions` (registration) or
    /// `CredentialRequestOptions` (sign-in), binary fields as base64url.
    #[schema(value_type = Object)]
    pub options: serde_json::Value,
}

#[derive(Debug, Clone, Copy)]
enum Ceremony {
    Register,
    SignIn,
}

impl Ceremony {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Register => "register",
            Self::SignIn => "sign_in",
        }
    }
}

fn failed() -> ApiError {
    ApiError::Unauthorized("passkey_failed")
}

fn challenge_expired() -> ApiError {
    ApiError::Gone("passkey_challenge_expired")
}

fn not_found() -> ApiError {
    ApiError::NotFound("passkey_not_found")
}

/// A verification webauthn-rs refused: the client sees only `passkey_failed`.
fn refused(error: &impl std::fmt::Display) -> ApiError {
    tracing::info!(%error, "passkey verification failed");
    failed()
}

fn internal(context: &'static str) -> impl FnOnce(webauthn_rs::prelude::WebauthnError) -> ApiError {
    move |error| ApiError::Internal(anyhow::anyhow!("{context}: {error}"))
}

/// Store the state of a ceremony just started, after deleting every expired
/// one, and pair its id with the options for the browser.
async fn store_challenge(
    db: &PgPool,
    ceremony: Ceremony,
    user: Option<UserId>,
    state: &impl Serialize,
    options: &impl Serialize,
    now: OffsetDateTime,
) -> Result<PasskeyChallenge, ApiError> {
    let state = serde_json::to_value(state).context("serialising the WebAuthn state")?;
    let options = serde_json::to_value(options).context("serialising the WebAuthn options")?;
    let mut tx = db.begin().await?;
    sqlx::query!(
        "DELETE FROM webauthn_challenges WHERE expires_at <= $1",
        now
    )
    .execute(&mut *tx)
    .await?;
    let challenge_id = sqlx::query_scalar!(
        "INSERT INTO webauthn_challenges (kind, user_id, state, expires_at)
         VALUES ($1, $2, $3, $4) RETURNING id",
        ceremony.as_str(),
        user.map(|user| user.0),
        state,
        now + CHALLENGE_LIFETIME,
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(PasskeyChallenge {
        challenge_id,
        options,
    })
}

/// Take the state of challenge `id` back: delete the row and return its state
/// when it belongs to `ceremony` and `user` and has not expired. It runs on
/// its own, outside the finish's transaction, so the challenge stays used
/// up even when the verification fails.
///
/// # Errors
/// 410 `passkey_challenge_expired` for an unknown, expired or mismatched
/// challenge; that leaves the row alone.
async fn take_challenge<T: DeserializeOwned>(
    db: &PgPool,
    id: Uuid,
    ceremony: Ceremony,
    user: Option<UserId>,
    now: OffsetDateTime,
) -> Result<T, ApiError> {
    let state = sqlx::query_scalar!(
        "DELETE FROM webauthn_challenges
         WHERE id = $1 AND kind = $2 AND user_id IS NOT DISTINCT FROM $3 AND expires_at > $4
         RETURNING state",
        id,
        ceremony.as_str(),
        user.map(|user| user.0),
        now,
    )
    .fetch_optional(db)
    .await?
    .ok_or_else(challenge_expired)?;
    Ok(serde_json::from_value(state).context("reading the WebAuthn state")?)
}

/// Lock an active User for the rest of the transaction. `None` when there
/// is no such User or they are deactivated.
async fn lock_active_user(
    conn: &mut PgConnection,
    id: UserId,
) -> anyhow::Result<Option<Identified>> {
    let row = sqlx::query!(
        "SELECT username, display_name, is_active FROM users WHERE id = $1 FOR NO KEY UPDATE",
        id.0,
    )
    .fetch_optional(conn)
    .await
    .context("locking the user")?;
    Ok(row.filter(|row| row.is_active).map(|row| Identified {
        id,
        username: row.username,
        display_name: row.display_name,
    }))
}

/// A stored Passkey, locked for the rest of the transaction.
struct Locked {
    id: PasskeyId,
    passkey: Passkey,
}

/// Lock `user`'s Passkey with `credential_id`; `None` when they have none.
async fn lock_passkey(
    conn: &mut PgConnection,
    user: UserId,
    credential_id: &[u8],
) -> anyhow::Result<Option<Locked>> {
    let Some(row) = sqlx::query!(
        r#"SELECT id AS "id: PasskeyId", passkey FROM passkeys
           WHERE user_id = $1 AND credential_id = $2
           FOR UPDATE"#,
        user.0,
        credential_id,
    )
    .fetch_optional(&mut *conn)
    .await
    .context("locking the passkey")?
    else {
        return Ok(None);
    };
    let passkey = serde_json::from_value(row.passkey).context("reading a stored passkey")?;
    Ok(Some(Locked {
        id: row.id,
        passkey,
    }))
}

/// Store what a successful use changed (the signature counter, the backup
/// flags) and when it happened.
async fn record_use(
    conn: &mut PgConnection,
    mut locked: Locked,
    result: &AuthenticationResult,
    now: OffsetDateTime,
) -> anyhow::Result<()> {
    locked.passkey.update_credential(result);
    let passkey = serde_json::to_value(&locked.passkey).context("serialising a passkey")?;
    sqlx::query!(
        "UPDATE passkeys SET passkey = $2, last_used_at = $3 WHERE id = $1",
        locked.id.0,
        passkey,
        now,
    )
    .execute(conn)
    .await
    .context("recording a passkey use")?;
    Ok(())
}

/// The signed-in User's Passkeys, oldest first.
///
/// # Errors
/// Fails when the query fails.
pub async fn list(db: &PgPool, user: UserId) -> anyhow::Result<Vec<PasskeySummary>> {
    sqlx::query_as!(
        PasskeySummary,
        r#"SELECT id AS "id: PasskeyId", name, created_at, last_used_at
           FROM passkeys WHERE user_id = $1 ORDER BY created_at, id"#,
        user.0,
    )
    .fetch_all(db)
    .await
    .context("listing passkeys")
}

/// Start registering a Passkey for `user`. The credential must be
/// discoverable (username-less sign-in needs it), and the authenticator is
/// told which credentials the User has already, so it never registers twice.
///
/// # Errors
/// `Internal` when the query or webauthn-rs fails.
pub async fn start_registration(
    db: &PgPool,
    webauthn: &Webauthn,
    user: &CurrentUser,
    now: OffsetDateTime,
) -> Result<PasskeyChallenge, ApiError> {
    let known = sqlx::query_scalar!(
        "SELECT credential_id FROM passkeys WHERE user_id = $1 ORDER BY created_at, id",
        user.id.0,
    )
    .fetch_all(db)
    .await?;
    let exclude = (!known.is_empty()).then_some(known);
    let (mut options, state) = webauthn
        .start_passkey_registration(user.id.0, &user.username, &user.display_name, exclude)
        .map_err(internal("starting a passkey registration"))?;
    require_resident_key(&mut options);
    store_challenge(db, Ceremony::Register, Some(user.id), &state, &options, now).await
}

fn require_resident_key(options: &mut CreationChallengeResponse) {
    let selection =
        options
            .public_key
            .authenticator_selection
            .get_or_insert(AuthenticatorSelectionCriteria {
                authenticator_attachment: None,
                resident_key: None,
                require_resident_key: false,
                user_verification: UserVerificationPolicy::Required,
            });
    selection.resident_key = Some(ResidentKeyRequirement::Required);
    selection.require_resident_key = true;
}

fn validate_name(name: &str) -> Result<&str, ApiError> {
    let name = name.trim();
    if (1..=NAME_MAX).contains(&name.chars().count()) {
        Ok(name)
    } else {
        Err(ApiError::Validation(vec![FieldError {
            field: "name".to_owned(),
            code: "name_length".to_owned(),
        }]))
    }
}

/// Finish a registration `user` started and store the Passkey as `name`.
///
/// # Errors
/// 422 `validation_failed` (field `name`, code `name_length`); 410
/// `passkey_challenge_expired`; 401 `passkey_failed` when the credential does
/// not verify, `unauthenticated` when the User was deactivated meanwhile;
/// 409 `passkey_exists` when the credential is registered already.
pub async fn finish_registration(
    db: &PgPool,
    webauthn: &Webauthn,
    user: &CurrentUser,
    challenge_id: Uuid,
    name: &str,
    credential: &RegisterPublicKeyCredential,
    now: OffsetDateTime,
) -> Result<PasskeySummary, ApiError> {
    let name = validate_name(name)?;
    let state: PasskeyRegistration =
        take_challenge(db, challenge_id, Ceremony::Register, Some(user.id), now).await?;
    let passkey = webauthn
        .finish_passkey_registration(credential, &state)
        .map_err(|error| refused(&error))?;
    let stored = serde_json::to_value(&passkey).context("serialising a passkey")?;

    let mut tx = db.begin().await?;
    // A deactivation that committed after the session was read ends here.
    lock_active_user(&mut tx, user.id)
        .await?
        .ok_or(ApiError::Unauthorized("unauthenticated"))?;
    let summary = sqlx::query_as!(
        PasskeySummary,
        r#"INSERT INTO passkeys (user_id, credential_id, name, passkey, created_at)
           VALUES ($1, $2, $3, $4, $5)
           ON CONFLICT (credential_id) DO NOTHING
           RETURNING id AS "id: PasskeyId", name, created_at, last_used_at"#,
        user.id.0,
        passkey.cred_id().as_slice(),
        name,
        stored,
        now,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Conflict("passkey_exists"))?;
    audit::record(
        &mut tx,
        NewAuditEntry {
            actor: Some(user.id),
            action: AuditAction::Create,
            object_type: "passkey",
            object_id: summary.id.0.to_string(),
            summary: format!("Registered passkey {name}"),
            changes: Vec::new(),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(summary)
}

/// Delete one of the signed-in User's own Passkeys.
///
/// # Errors
/// 404 `passkey_not_found` when `user` has no Passkey `id`; 409
/// `last_administrator` when it is the last way the last Administrator who
/// can sign in has to do so.
pub async fn delete_own(db: &PgPool, user: &CurrentUser, id: PasskeyId) -> Result<(), ApiError> {
    let mut tx = db.begin().await?;
    guard_own_passkey_removal(&mut tx, user.id, id).await?;
    let name = sqlx::query_scalar!(
        "DELETE FROM passkeys WHERE id = $1 AND user_id = $2 RETURNING name",
        id.0,
        user.id.0,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(not_found)?;
    audit::record(
        &mut tx,
        NewAuditEntry {
            actor: Some(user.id),
            action: AuditAction::Delete,
            object_type: "passkey",
            object_id: id.0.to_string(),
            summary: format!("Deleted passkey {name}"),
            changes: Vec::new(),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Start a username-less sign-in: the browser offers the discoverable
/// credentials it holds for this site.
///
/// # Errors
/// `Internal` when the query or webauthn-rs fails.
pub async fn start_sign_in(
    db: &PgPool,
    webauthn: &Webauthn,
    now: OffsetDateTime,
) -> Result<PasskeyChallenge, ApiError> {
    let (options, state) = webauthn
        .start_discoverable_authentication()
        .map_err(internal("starting a passkey sign-in"))?;
    store_challenge(db, Ceremony::SignIn, None, &state, &options, now).await
}

/// A sign-in challenge taken back by [`take_sign_in_challenge`].
pub struct SignInChallenge(DiscoverableAuthentication);

/// Take back the state of a sign-in challenge, using it up.
///
/// # Errors
/// 410 `passkey_challenge_expired`.
pub async fn take_sign_in_challenge(
    db: &PgPool,
    id: Uuid,
    now: OffsetDateTime,
) -> Result<SignInChallenge, ApiError> {
    take_challenge(db, id, Ceremony::SignIn, None, now)
        .await
        .map(SignInChallenge)
}

/// Verify a username-less sign-in: find the User the credential names,
/// check the response against that User's Passkey with this credential id,
/// and record the use. The session itself is the caller's, in the same
/// transaction.
///
/// # Errors
/// 401 `passkey_failed` for an unknown credential or User, a deactivated
/// User, or a response that does not verify.
pub async fn verify_sign_in(
    conn: &mut PgConnection,
    webauthn: &Webauthn,
    challenge: SignInChallenge,
    credential: &PublicKeyCredential,
    now: OffsetDateTime,
) -> Result<Identified, ApiError> {
    let (user, credential_id) = webauthn
        .identify_discoverable_authentication(credential)
        .map_err(|error| refused(&error))?;
    let user = lock_active_user(conn, UserId(user))
        .await?
        .ok_or_else(failed)?;
    // The user handle is not signed, so the credential must belong to the
    // User it names: the lookup takes both.
    let locked = lock_passkey(conn, user.id, credential_id)
        .await?
        .ok_or_else(failed)?;
    let result = webauthn
        .finish_discoverable_authentication_v2(credential, challenge.0, &locked.passkey)
        .map_err(|error| refused(&error))?;
    record_use(conn, locked, &result, now).await?;
    Ok(user)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_trimmed_and_one_to_fifty_characters() {
        assert_eq!(validate_name("  Ponsel  ").unwrap(), "Ponsel");
        assert_eq!(validate_name(&"é".repeat(50)).unwrap(), "é".repeat(50));
        assert!(validate_name("   ").is_err());
        assert!(validate_name(&"é".repeat(51)).is_err());
    }

    #[test]
    fn the_relying_party_follows_the_public_origin() {
        let origin: Url = "https://desa.example.id".parse().unwrap();
        let webauthn = relying_party(&origin).unwrap();
        assert_eq!(webauthn.get_allowed_origins(), [origin]);
    }
}
