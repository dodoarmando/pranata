use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use anyhow::Context;
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{AppendHeaders, IntoResponse},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, types::Json as Jsonb};
use utoipa::ToSchema;
use uuid::Uuid;
use webauthn_rs::{
    DEFAULT_AUTHENTICATOR_TIMEOUT, Webauthn, WebauthnBuilder,
    fake::{FakePasskeyDistribution, WebauthnFakeCredentialGenerator},
    prelude::{
        Passkey, PasskeyAuthentication, PasskeyRegistration, PublicKeyCredential,
        RegisterPublicKeyCredential, Url,
    },
};
use webauthn_rs_proto::AllowCredentials;

use crate::{
    ApiError,
    audit::{self, Actor},
    auth::{self, Session},
};

/// Ceremonies in flight; more are refused, so a flood of challenges cannot exhaust memory.
const MAX_CEREMONIES: usize = 10_000;

/// WebAuthn ceremony state, which must stay on the server, keyed by a random ceremony id and
/// usable once within the authenticator timeout.
// shortcut: in-memory state assumes a single API process (ADR-0001); if the API ever scales
// out, enable webauthn-rs `danger-allow-state-serialisation` and keep it in Postgres.
struct Ceremonies<T>(Mutex<HashMap<Uuid, (Instant, T)>>);

impl<T> Ceremonies<T> {
    fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    /// Keeps `state` and returns its ceremony id; `None` when too many are in flight.
    fn start(&self, state: T) -> Option<Uuid> {
        let mut ceremonies = self.0.lock().expect("ceremony lock poisoned");
        ceremonies.retain(|_, (started, _)| started.elapsed() < DEFAULT_AUTHENTICATOR_TIMEOUT);
        if ceremonies.len() >= MAX_CEREMONIES {
            return None;
        }
        let id = Uuid::new_v4();
        ceremonies.insert(id, (Instant::now(), state));
        Some(id)
    }

    /// Takes the state of a live ceremony, which cannot be finished again.
    fn finish(&self, id: Uuid) -> Option<T> {
        let mut ceremonies = self.0.lock().expect("ceremony lock poisoned");
        let (started, state) = ceremonies.remove(&id)?;
        (started.elapsed() < DEFAULT_AUTHENTICATOR_TIMEOUT).then_some(state)
    }
}

/// The Installation's relying party and its ceremonies in flight.
pub(crate) struct Passkeys {
    webauthn: Webauthn,
    /// Each registration with the User who started it.
    registrations: Ceremonies<(Uuid, PasskeyRegistration)>,
    /// Each sign-in with the User it is for, or `None` for a decoy.
    sign_ins: Ceremonies<Option<(Uuid, PasskeyAuthentication)>>,
    /// The same believable credentials for each username without passkeys, every time.
    // shortcut: the key is drawn at start-up, so decoys change on a restart while real
    // credentials do not; persist it if that ever matters.
    decoys: WebauthnFakeCredentialGenerator<FakePasskeyDistribution>,
}

impl Passkeys {
    /// Binds passkeys to the domain of `public_origin`.
    pub(crate) fn new(public_origin: &str) -> anyhow::Result<Self> {
        let origin = Url::parse(public_origin)
            .with_context(|| format!("the public origin {public_origin:?} is not a URL"))?;
        let rp_id = origin
            .domain()
            .with_context(|| format!("the public origin {public_origin:?} has no domain"))?;
        let webauthn = WebauthnBuilder::new(rp_id, &origin)
            .and_then(WebauthnBuilder::build)
            .context("cannot set up passkeys")?;
        let mut key = [0u8; 32];
        getrandom::fill(&mut key).context("the OS random number generator is unavailable")?;
        Ok(Self {
            webauthn,
            registrations: Ceremonies::new(),
            sign_ins: Ceremonies::new(),
            decoys: WebauthnFakeCredentialGenerator::new(&key)?,
        })
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PasskeySummary {
    id: Uuid,
    name: String,
    created_at: DateTime<Utc>,
    /// When it last signed the User in; null if never.
    last_used_at: Option<DateTime<Utc>>,
}

/// The signed-in User's passkeys, oldest first.
///
/// # Errors
/// `401` without a live session.
#[utoipa::path(
    get,
    path = "/auth/passkeys",
    operation_id = "listPasskeys",
    tag = "auth",
    responses(
        (status = OK, description = "The User's passkeys", body = Vec<PasskeySummary>),
        (status = UNAUTHORIZED, description = "There is no live session"),
    ),
)]
pub(crate) async fn list(
    State(pool): State<PgPool>,
    session: Session,
) -> Result<Json<Vec<PasskeySummary>>, ApiError> {
    let passkeys = sqlx::query_as!(
        PasskeySummary,
        "SELECT id, name, created_at, last_used_at FROM passkeys WHERE user_id = $1 ORDER BY id",
        session.user_id,
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(passkeys))
}

/// A WebAuthn ceremony's options for the browser, and its id to finish it with.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PasskeyChallenge {
    ceremony_id: Uuid,
    /// The ceremony options as JSON; `publicKey` goes to `PublicKeyCredential.parse…OptionsFromJSON`.
    options: Value,
}

/// Starts adding a passkey for the signed-in User, excluding the ones they already have.
///
/// # Errors
/// `401` without a live session, `428` outside sudo mode, `503` with too many ceremonies in flight.
#[utoipa::path(
    post,
    path = "/auth/passkeys/registration",
    operation_id = "startPasskeyRegistration",
    tag = "auth",
    responses(
        (status = OK, description = "The options for `navigator.credentials.create`", body = PasskeyChallenge),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = PRECONDITION_REQUIRED, description = "The User must re-authenticate first"),
        (status = SERVICE_UNAVAILABLE, description = "Too many ceremonies are in flight; try again"),
    ),
)]
pub(crate) async fn start_registration(
    State(pool): State<PgPool>,
    State(passkeys): State<Arc<Passkeys>>,
    session: Session,
) -> Result<Json<PasskeyChallenge>, ApiError> {
    session.ensure_sudo()?;
    let existing = sqlx::query_scalar!(
        "SELECT credential_id FROM passkeys WHERE user_id = $1",
        session.user_id
    )
    .fetch_all(&pool)
    .await?;
    let (options, state) = passkeys.webauthn.start_passkey_registration(
        session.user_id,
        &session.username,
        &session.username,
        Some(existing),
    )?;
    let ceremony_id = passkeys
        .registrations
        .start((session.user_id, state))
        .ok_or(ApiError::Status(StatusCode::SERVICE_UNAVAILABLE))?;
    Ok(Json(PasskeyChallenge {
        ceremony_id,
        options: serde_json::to_value(options)?,
    }))
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewPasskey {
    ceremony_id: Uuid,
    /// 1–64 characters, such as the device it lives on.
    name: String,
    /// `PublicKeyCredential.toJSON()` of the created credential.
    #[schema(value_type = Object)]
    credential: RegisterPublicKeyCredential,
}

/// Finishes adding a passkey for the signed-in User.
///
/// # Errors
/// `400` for a credential that fails verification, `401` without a live session, `404` for an
/// unknown, used or expired ceremony or one another User started, `409` for a credential
/// already registered, `422` for a missing or over-long name, `428` outside sudo mode.
#[utoipa::path(
    post,
    path = "/auth/passkeys",
    operation_id = "createPasskey",
    tag = "auth",
    request_body = NewPasskey,
    responses(
        (status = CREATED, description = "The passkey is added", body = PasskeySummary),
        (status = BAD_REQUEST, description = "The credential fails verification"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = NOT_FOUND, description = "The ceremony is unknown, used or expired"),
        (status = CONFLICT, description = "The credential is already registered"),
        (status = UNPROCESSABLE_ENTITY, description = "The name is missing or too long"),
        (status = PRECONDITION_REQUIRED, description = "The User must re-authenticate first"),
    ),
)]
pub(crate) async fn create(
    State(pool): State<PgPool>,
    State(passkeys): State<Arc<Passkeys>>,
    session: Session,
    Json(body): Json<NewPasskey>,
) -> Result<(StatusCode, Json<PasskeySummary>), ApiError> {
    session.ensure_sudo()?;
    let (user_id, state) = passkeys
        .registrations
        .finish(body.ceremony_id)
        .filter(|(user_id, _)| *user_id == session.user_id)
        .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    let name = body.name.trim();
    if !(1..=64).contains(&name.chars().count()) {
        return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
    }
    // Checked first, since webauthn-rs refuses the User's own with a misleading error.
    let registered = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT FROM passkeys WHERE credential_id = $1) AS "registered!""#,
        body.credential.raw_id.as_slice(),
    )
    .fetch_one(&pool)
    .await?;
    if registered {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    let passkey: Passkey = passkeys
        .webauthn
        .finish_passkey_registration(&body.credential, &state)
        .map_err(|_| ApiError::Status(StatusCode::BAD_REQUEST))?;
    let mut tx = pool.begin().await?;
    let created = sqlx::query_as!(
        PasskeySummary,
        "INSERT INTO passkeys (user_id, credential_id, passkey, name) VALUES ($1, $2, $3, $4)
         RETURNING id, name, created_at, last_used_at",
        user_id,
        passkey.cred_id().as_slice(),
        Jsonb(&passkey) as _,
        name,
    )
    .fetch_one(&mut *tx)
    .await;
    let created = match created {
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            return Err(ApiError::Status(StatusCode::CONFLICT));
        }
        created => created?,
    };
    audit::record(
        &mut tx,
        Actor::User(user_id),
        "passkey.create",
        ("user", Some(&user_id.to_string())),
        json!({ "name": name }),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// Removes one of the signed-in User's passkeys.
///
/// # Errors
/// `401` without a live session, `404` for a passkey that is not theirs, `428` outside sudo mode.
#[utoipa::path(
    delete,
    path = "/auth/passkeys/{id}",
    operation_id = "deletePasskey",
    tag = "auth",
    params(("id" = Uuid, Path, description = "The passkey")),
    responses(
        (status = NO_CONTENT, description = "The passkey is removed"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = NOT_FOUND, description = "The User has no such passkey"),
        (status = PRECONDITION_REQUIRED, description = "The User must re-authenticate first"),
    ),
)]
pub(crate) async fn delete(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    session.ensure_sudo()?;
    let mut tx = pool.begin().await?;
    let name = sqlx::query_scalar!(
        "DELETE FROM passkeys WHERE id = $1 AND user_id = $2 RETURNING name",
        id,
        session.user_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    audit::record(
        &mut tx,
        Actor::User(session.user_id),
        "passkey.delete",
        ("user", Some(&session.user_id.to_string())),
        json!({ "name": name }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub struct PasskeySignInStart {
    username: String,
}

/// Starts signing in with a passkey for a username. A username that is unknown, inactive or
/// without passkeys gets a decoy challenge, so the answer never tells whether it exists.
///
/// # Errors
/// `503` with too many ceremonies in flight.
#[utoipa::path(
    post,
    path = "/auth/passkey-sign-in/challenge",
    operation_id = "startPasskeySignIn",
    tag = "auth",
    request_body = PasskeySignInStart,
    responses(
        (status = OK, description = "The options for `navigator.credentials.get`", body = PasskeyChallenge),
        (status = SERVICE_UNAVAILABLE, description = "Too many ceremonies are in flight; try again"),
    ),
)]
pub(crate) async fn start_sign_in(
    State(pool): State<PgPool>,
    State(passkeys): State<Arc<Passkeys>>,
    Json(body): Json<PasskeySignInStart>,
) -> Result<Json<PasskeyChallenge>, ApiError> {
    let rows = sqlx::query!(
        r#"SELECT user_id, passkey AS "passkey: Jsonb<Passkey>" FROM passkeys
           JOIN users ON users.id = user_id WHERE username = $1 AND active"#,
        body.username,
    )
    .fetch_all(&pool)
    .await?;
    let user_id = rows.first().map(|row| row.user_id);
    let credentials: Vec<Passkey> = rows.into_iter().map(|row| row.passkey.0).collect();
    let (mut options, state) = passkeys
        .webauthn
        .start_passkey_authentication(&credentials)?;
    let state = if let Some(user_id) = user_id {
        Some((user_id, state))
    } else {
        options.public_key.allow_credentials = passkeys
            .decoys
            .generate(body.username.as_bytes())?
            .into_iter()
            .map(|id| AllowCredentials {
                type_: "public-key".to_owned(),
                id,
                // As webauthn-rs sends real ones.
                transports: None,
            })
            .collect();
        None
    };
    let ceremony_id = passkeys
        .sign_ins
        .start(state)
        .ok_or(ApiError::Status(StatusCode::SERVICE_UNAVAILABLE))?;
    Ok(Json(PasskeyChallenge {
        ceremony_id,
        options: serde_json::to_value(options)?,
    }))
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PasskeySignIn {
    ceremony_id: Uuid,
    /// `PublicKeyCredential.toJSON()` of the assertion.
    #[schema(value_type = Object)]
    credential: PublicKeyCredential,
    /// "Ingat saya", as for a password sign-in.
    remember: bool,
}

/// Signs in with a passkey instead of the password, starting a session held in the
/// `__Host-session` cookie.
///
/// # Errors
/// `401`, the same for a decoy, an unknown, used or expired ceremony, a failed assertion and an
/// inactive User.
#[utoipa::path(
    post,
    path = "/auth/passkey-sign-in",
    operation_id = "signInWithPasskey",
    tag = "auth",
    request_body = PasskeySignIn,
    responses(
        (status = NO_CONTENT, description = "Signed in; the response sets the session cookie"),
        (status = UNAUTHORIZED, description = "The passkey does not sign this User in"),
    ),
)]
pub(crate) async fn finish_sign_in(
    State(pool): State<PgPool>,
    State(passkeys): State<Arc<Passkeys>>,
    headers: HeaderMap,
    Json(body): Json<PasskeySignIn>,
) -> Result<impl IntoResponse, ApiError> {
    let ceremony = passkeys.sign_ins.finish(body.ceremony_id).flatten();
    let attempted = ceremony.as_ref().map(|(user_id, _)| user_id.to_string());
    let verified = ceremony.and_then(|(user_id, state)| {
        let result = passkeys
            .webauthn
            .finish_passkey_authentication(&body.credential, &state)
            .ok()?;
        Some((user_id, result))
    });
    let mut tx = pool.begin().await?;
    let stored = match &verified {
        // The User may have been deactivated, or the passkey removed, since the challenge.
        Some((user_id, result)) => {
            sqlx::query!(
                r#"SELECT passkeys.id, name, passkey AS "passkey: Jsonb<Passkey>" FROM passkeys
                   JOIN users ON users.id = user_id
                   WHERE user_id = $1 AND credential_id = $2 AND active FOR UPDATE OF passkeys"#,
                user_id,
                result.cred_id().as_slice(),
            )
            .fetch_optional(&mut *tx)
            .await?
        }
        None => None,
    };
    let (Some((user_id, result)), Some(stored)) = (verified, stored) else {
        audit::record(
            &mut tx,
            Actor::Anonymous,
            "session.sign_in_failed",
            ("user", attempted.as_deref()),
            json!({}),
        )
        .await?;
        tx.commit().await?;
        return Err(ApiError::Status(StatusCode::UNAUTHORIZED));
    };

    // Keeps the signature counter, which flags a cloned authenticator next time.
    let mut passkey = stored.passkey.0;
    passkey.update_credential(&result);
    sqlx::query!(
        "UPDATE passkeys SET passkey = $2, last_used_at = now() WHERE id = $1",
        stored.id,
        Jsonb(&passkey) as _,
    )
    .execute(&mut *tx)
    .await?;
    let cookie = auth::start_session(
        &mut tx,
        user_id,
        body.remember,
        &headers,
        json!({ "remember": body.remember, "passkey": stored.name }),
    )
    .await?;
    tx.commit().await?;
    Ok((
        StatusCode::NO_CONTENT,
        AppendHeaders([(header::SET_COOKIE, cookie)]),
    ))
}
