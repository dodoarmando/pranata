//! Passkey routes (D-30, D-32): register, list and delete your own Passkeys,
//! and sign in with one without a username. The ceremonies live in
//! [`crate::auth::passkey`].
//!
//! Everything under `/api/auth/passkeys` needs the CSRF header ([`crate::auth::csrf`]). The sign-in routes are mounted with the
//! other unauthenticated POSTs in [`super::auth::unauthenticated_router`],
//! behind the per-IP limit.

use axum::{
    Json,
    extract::{Path, State, rejection::PathRejection},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;
use webauthn_rs::prelude::{PublicKeyCredential, RegisterPublicKeyCredential};

use crate::{
    ApiError, AppState,
    auth::{
        CurrentUser,
        passkey::{self, PasskeyChallenge, PasskeySummary},
        session::AuthSession,
    },
    error::Problem,
    http::{
        auth::{SessionInfo, begin_session, no_store},
        json::ApiJson,
    },
    ids::PasskeyId,
};

/// The answer to a registration challenge.
#[derive(Debug, Deserialize, ToSchema)]
pub struct FinishPasskeyRegistrationRequest {
    pub challenge_id: Uuid,
    /// 1 to 50 characters after trimming, e.g. the device it lives on.
    pub name: String,
    /// The `PublicKeyCredential` from `navigator.credentials.create()`, as
    /// JSON with binary fields in base64url.
    #[schema(value_type = Object)]
    pub credential: RegisterPublicKeyCredential,
}

/// The answer to a sign-in challenge.
#[derive(Debug, Deserialize, ToSchema)]
pub struct FinishPasskeySignInRequest {
    pub challenge_id: Uuid,
    /// The `PublicKeyCredential` from `navigator.credentials.get()`, as JSON
    /// with binary fields in base64url.
    #[schema(value_type = Object)]
    pub credential: PublicKeyCredential,
    /// Keep the session for 30 days instead of until the browser closes.
    #[serde(default)]
    pub remember: bool,
}

/// The signed-in User's Passkeys, oldest first.
///
/// # Errors
/// 401 `unauthenticated`.
#[utoipa::path(
    get,
    path = "/auth/passkeys",
    operation_id = "listPasskeys",
    tag = "passkeys",
    security(("session" = [])),
    responses(
        (status = 200, description = "The signed-in User's Passkeys", body = Vec<PasskeySummary>),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn list_passkeys(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<PasskeySummary>>, ApiError> {
    Ok(Json(passkey::list(&state.db, user.id).await?))
}

/// Start registering a Passkey for the signed-in User.
///
/// # Errors
/// 401 `unauthenticated`; 403 `csrf_mismatch` or `sudo_required`.
#[utoipa::path(
    post,
    path = "/auth/passkeys/registration/start",
    operation_id = "startPasskeyRegistration",
    tag = "passkeys",
    params(("x-csrf-token" = String, Header, description = "The CSRF token from sign-in")),
    security(("session" = [])),
    responses(
        (status = 200, description = "The challenge and the options for navigator.credentials.create() (not cached)", body = PasskeyChallenge),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Missing or wrong CSRF token (code csrf_mismatch) or Sudo Mode is closed (code sudo_required)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn start_passkey_registration(
    State(state): State<AppState>,
    session: AuthSession,
) -> Result<impl IntoResponse, ApiError> {
    let now = state.clock.now();
    session.require_sudo(now)?;
    let challenge =
        passkey::start_registration(&state.db, &state.webauthn, &session.user, now).await?;
    Ok((no_store(), Json(challenge)))
}

/// Finish registering a Passkey and store it under a name.
///
/// # Errors
/// 401 `unauthenticated` or `passkey_failed`; 403 `csrf_mismatch`; 410
/// `passkey_challenge_expired`; 422 `validation_failed`; 409 `passkey_exists`.
#[utoipa::path(
    post,
    path = "/auth/passkeys/registration/finish",
    operation_id = "finishPasskeyRegistration",
    tag = "passkeys",
    params(("x-csrf-token" = String, Header, description = "The CSRF token from sign-in")),
    security(("session" = [])),
    request_body = FinishPasskeyRegistrationRequest,
    responses(
        (status = 201, description = "The new Passkey", body = PasskeySummary),
        (status = 400, description = "Body is not valid JSON (code invalid_json)", body = Problem, content_type = "application/problem+json"),
        (status = 401, description = "Not signed in (code unauthenticated) or the credential does not verify (code passkey_failed)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Missing or wrong CSRF token (code csrf_mismatch)", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The credential is registered already (code passkey_exists)", body = Problem, content_type = "application/problem+json"),
        (status = 410, description = "The challenge is unknown, used or expired, or belongs to another ceremony or User (code passkey_challenge_expired)", body = Problem, content_type = "application/problem+json"),
        (status = 415, description = "Request is not marked as JSON (code json_required)", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid fields (code validation_failed; field code name_length)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn finish_passkey_registration(
    State(state): State<AppState>,
    user: CurrentUser,
    ApiJson(request): ApiJson<FinishPasskeyRegistrationRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let summary = passkey::finish_registration(
        &state.db,
        &state.webauthn,
        &user,
        request.challenge_id,
        &request.name,
        &request.credential,
        state.clock.now(),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(summary)))
}

/// Delete one of the signed-in User's Passkeys.
///
/// # Errors
/// 401 `unauthenticated`; 403 `csrf_mismatch` or `sudo_required`; 404
/// `passkey_not_found`; 409 `last_administrator`.
#[utoipa::path(
    delete,
    path = "/auth/passkeys/{id}",
    operation_id = "deletePasskey",
    tag = "passkeys",
    params(
        ("id" = Uuid, Path, description = "Passkey id"),
        ("x-csrf-token" = String, Header, description = "The CSRF token from sign-in"),
    ),
    security(("session" = [])),
    responses(
        (status = 204, description = "The Passkey is deleted"),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Missing or wrong CSRF token (code csrf_mismatch) or Sudo Mode is closed (code sudo_required)", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "The signed-in User has no such Passkey (code passkey_not_found)", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "It is the only way left for the last Administrator who can sign in (code last_administrator)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn delete_passkey(
    State(state): State<AppState>,
    session: AuthSession,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<StatusCode, ApiError> {
    session.require_sudo(state.clock.now())?;
    let Ok(Path(id)) = id else {
        return Err(ApiError::NotFound("passkey_not_found"));
    };
    passkey::delete_own(&state.db, &session.user, PasskeyId(id)).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Start a sign-in with a Passkey, without a username.
///
/// # Errors
/// 429 `too_many_requests`.
#[utoipa::path(
    post,
    path = "/auth/passkey-login/start",
    operation_id = "startPasskeySignIn",
    tag = "auth",
    responses(
        (status = 200, description = "The challenge and the options for navigator.credentials.get() (not cached)", body = PasskeyChallenge),
        (status = 429, description = "Too many requests from this address (code too_many_requests); Retry-After gives the seconds to wait", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn start_passkey_sign_in(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let challenge = passkey::start_sign_in(&state.db, &state.webauthn, state.clock.now()).await?;
    Ok((no_store(), Json(challenge)))
}

/// Finish a sign-in with a Passkey. Like `login`, it sets the session cookie.
///
/// # Errors
/// 401 `passkey_failed`; 410 `passkey_challenge_expired`; 429
/// `too_many_requests`.
#[utoipa::path(
    post,
    path = "/auth/passkey-login/finish",
    operation_id = "finishPasskeySignIn",
    tag = "auth",
    request_body = FinishPasskeySignInRequest,
    responses(
        (status = 200, description = "Signed in; the session cookie is set", body = SessionInfo),
        (status = 400, description = "Body is not valid JSON (code invalid_json)", body = Problem, content_type = "application/problem+json"),
        (status = 401, description = "Unknown credential, deactivated User or a response that does not verify (code passkey_failed)", body = Problem, content_type = "application/problem+json"),
        (status = 410, description = "The challenge is unknown, used or expired, or belongs to another ceremony (code passkey_challenge_expired)", body = Problem, content_type = "application/problem+json"),
        (status = 415, description = "Request is not marked as JSON (code json_required)", body = Problem, content_type = "application/problem+json"),
        (status = 429, description = "Too many requests from this address (code too_many_requests); Retry-After gives the seconds to wait", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn finish_passkey_sign_in(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(request): ApiJson<FinishPasskeySignInRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let now = state.clock.now();
    let challenge = passkey::take_sign_in_challenge(&state.db, request.challenge_id, now).await?;
    let mut tx = state.db.begin().await?;
    let user = passkey::verify_sign_in(
        &mut tx,
        &state.webauthn,
        challenge,
        &request.credential,
        now,
    )
    .await?;
    let summary = format!("{} signed in with a passkey", user.username);
    let signed_in = begin_session(&mut tx, &headers, user, request.remember, now, summary).await?;
    tx.commit().await?;
    Ok(signed_in)
}

/// Routes that need a session. Passkey sign-in is in
/// [`super::auth::unauthenticated_router`].
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_passkeys))
        .routes(routes!(start_passkey_registration))
        .routes(routes!(finish_passkey_registration))
        .routes(routes!(delete_passkey))
}
