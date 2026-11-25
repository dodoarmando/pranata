//! User administration routes. Every route needs `user.manage`; every write
//! also needs Sudo Mode. The escalation, validation and domain rules live in
//! [`crate::access::users`].

use axum::{
    Json,
    extract::{Path, State, rejection::PathRejection},
    http::StatusCode,
    response::IntoResponse,
};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{
    ApiError, AppState,
    access::{
        Permission,
        users::{
            self, AccessLinkIssued, CreateUserRequest, CreatedUser, UpdateUserRequest, UserSummary,
        },
    },
    auth::{CurrentUser, session::AuthSession},
    error::Problem,
    http::{auth::no_store, json::ApiJson},
    ids::UserId,
};

/// A malformed id names no User.
fn user_id(path: Result<Path<Uuid>, PathRejection>) -> Result<UserId, ApiError> {
    path.map(|Path(id)| UserId(id))
        .map_err(|_| ApiError::NotFound("user_not_found"))
}

/// Check `user.manage` and Sudo Mode, in that order.
fn require_write(state: &AppState, session: &AuthSession) -> Result<(), ApiError> {
    session.user.require(Permission::UserManage)?;
    session.require_sudo(state.clock.now())
}

/// Every User: active ones first, then by display name.
///
/// # Errors
/// 403 `permission_denied` without `user.manage`.
#[utoipa::path(
    get,
    path = "/admin/users",
    operation_id = "listUsers",
    tag = "users",
    security(("session" = [])),
    responses(
        (status = 200, description = "Every User", body = Vec<UserSummary>),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "The Role lacks user.manage (code permission_denied)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn list_users(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<UserSummary>>, ApiError> {
    user.require(Permission::UserManage)?;
    Ok(Json(users::list(&state.db).await?))
}

/// Add a User without a password and return their first Access Link. The link
/// is shown only in this response.
///
/// # Errors
/// 403 `permission_denied`, `sudo_required`, `administrator_only` or
/// `permission_not_held`; 422 `validation_failed`; 409 `username_taken`.
#[utoipa::path(
    post,
    path = "/admin/users",
    operation_id = "createUser",
    tag = "users",
    params(("x-csrf-token" = String, Header, description = "The CSRF token from sign-in")),
    security(("session" = [])),
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "The new User and their Access Link (not cached)", body = CreatedUser),
        (status = 400, description = "Body is not valid JSON (code invalid_json)", body = Problem, content_type = "application/problem+json"),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "The Role lacks user.manage (code permission_denied), Sudo Mode is closed (code sudo_required), or the Role may not be assigned by this User (codes administrator_only, permission_not_held)", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The username is taken, also by a deactivated User (code username_taken)", body = Problem, content_type = "application/problem+json"),
        (status = 415, description = "Request is not marked as JSON (code json_required)", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid fields (code validation_failed; field codes username_invalid, required, role_not_found)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn create_user(
    State(state): State<AppState>,
    session: AuthSession,
    body: Result<ApiJson<CreateUserRequest>, ApiError>,
) -> Result<impl IntoResponse, ApiError> {
    require_write(&state, &session)?;
    let ApiJson(request) = body?;
    let (user, link) = users::create(&state.db, &session.user, request, state.clock.now()).await?;
    let created = CreatedUser {
        user,
        access_link: AccessLinkIssued::new(&state.config.public_origin, &link),
    };
    Ok((StatusCode::CREATED, no_store(), Json(created)))
}

/// Change a User's display name or Role. The username never changes.
///
/// # Errors
/// 403 `permission_denied`, `sudo_required`, `administrator_only`,
/// `permission_not_held`, `stronger_user` or `own_role`; 422
/// `validation_failed`; 404
/// `user_not_found`; 409 `last_administrator`.
#[utoipa::path(
    patch,
    path = "/admin/users/{id}",
    operation_id = "updateUser",
    tag = "users",
    params(
        ("id" = Uuid, Path, description = "User id"),
        ("x-csrf-token" = String, Header, description = "The CSRF token from sign-in"),
    ),
    security(("session" = [])),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "The edited User", body = UserSummary),
        (status = 400, description = "Body is not valid JSON or has unknown fields such as username (code invalid_json)", body = Problem, content_type = "application/problem+json"),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "The Role lacks user.manage (code permission_denied), Sudo Mode is closed (code sudo_required), the new Role may not be assigned by this User (codes administrator_only, permission_not_held), the User is out of reach (codes administrator_only, stronger_user), or the User is changing their own Role (code own_role)", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such User (code user_not_found)", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "No other active Administrator who can sign in would remain (code last_administrator)", body = Problem, content_type = "application/problem+json"),
        (status = 415, description = "Request is not marked as JSON (code json_required)", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid fields (code validation_failed; field codes required, role_not_found)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn update_user(
    State(state): State<AppState>,
    session: AuthSession,
    id: Result<Path<Uuid>, PathRejection>,
    body: Result<ApiJson<UpdateUserRequest>, ApiError>,
) -> Result<Json<UserSummary>, ApiError> {
    require_write(&state, &session)?;
    let id = user_id(id)?;
    let ApiJson(request) = body?;
    Ok(Json(
        users::update(&state.db, &session.user, id, request).await?,
    ))
}

/// Deactivate a User: their sessions end and their unused Access Links stop
/// working. Users are never deleted, and the username is never reused.
///
/// # Errors
/// 403 `permission_denied`, `sudo_required`, `administrator_only` or
/// `stronger_user`; 404 `user_not_found`; 409 `cannot_deactivate_self`
/// or `last_administrator`.
#[utoipa::path(
    post,
    path = "/admin/users/{id}/deactivate",
    operation_id = "deactivateUser",
    tag = "users",
    params(
        ("id" = Uuid, Path, description = "User id"),
        ("x-csrf-token" = String, Header, description = "The CSRF token from sign-in"),
    ),
    security(("session" = [])),
    responses(
        (status = 200, description = "The deactivated User", body = UserSummary),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "The Role lacks user.manage (code permission_denied), Sudo Mode is closed (code sudo_required), or the User is out of reach (codes administrator_only, stronger_user)", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such User (code user_not_found)", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The User is the one signed in (code cannot_deactivate_self), or no other active Administrator who can sign in would remain (code last_administrator)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn deactivate_user(
    State(state): State<AppState>,
    session: AuthSession,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<UserSummary>, ApiError> {
    require_write(&state, &session)?;
    let id = user_id(id)?;
    Ok(Json(
        users::deactivate(&state.db, &session.user, id, state.clock.now()).await?,
    ))
}

/// Reactivate a User. They keep their password and Passkeys.
///
/// # Errors
/// 403 `permission_denied`, `sudo_required`, `administrator_only` or
/// `stronger_user`; 404 `user_not_found`.
#[utoipa::path(
    post,
    path = "/admin/users/{id}/reactivate",
    operation_id = "reactivateUser",
    tag = "users",
    params(
        ("id" = Uuid, Path, description = "User id"),
        ("x-csrf-token" = String, Header, description = "The CSRF token from sign-in"),
    ),
    security(("session" = [])),
    responses(
        (status = 200, description = "The reactivated User", body = UserSummary),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "The Role lacks user.manage (code permission_denied), Sudo Mode is closed (code sudo_required), or the User is out of reach (codes administrator_only, stronger_user)", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such User (code user_not_found)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn reactivate_user(
    State(state): State<AppState>,
    session: AuthSession,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<UserSummary>, ApiError> {
    require_write(&state, &session)?;
    let id = user_id(id)?;
    Ok(Json(users::reactivate(&state.db, &session.user, id).await?))
}

/// Issue a new Access Link for a User. Their earlier unused links stop
/// working. The link is shown only in this response.
///
/// # Errors
/// 403 `permission_denied`, `sudo_required`, `administrator_only` or
/// `stronger_user`; 404 `user_not_found`; 409 `user_inactive`.
#[utoipa::path(
    post,
    path = "/admin/users/{id}/access-links",
    operation_id = "issueAccessLink",
    tag = "users",
    params(
        ("id" = Uuid, Path, description = "User id"),
        ("x-csrf-token" = String, Header, description = "The CSRF token from sign-in"),
    ),
    security(("session" = [])),
    responses(
        (status = 201, description = "The new Access Link (not cached)", body = AccessLinkIssued),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "The Role lacks user.manage (code permission_denied), Sudo Mode is closed (code sudo_required), or the User is out of reach (codes administrator_only, stronger_user)", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such User (code user_not_found)", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The User is deactivated (code user_inactive)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn issue_access_link(
    State(state): State<AppState>,
    session: AuthSession,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<impl IntoResponse, ApiError> {
    require_write(&state, &session)?;
    let id = user_id(id)?;
    let link = users::issue_access_link(&state.db, &session.user, id, state.clock.now()).await?;
    Ok((
        StatusCode::CREATED,
        no_store(),
        Json(AccessLinkIssued::new(&state.config.public_origin, &link)),
    ))
}

/// Remove every Passkey of a User, after an account takeover for example:
/// a Passkey an attacker registered must not outlive a new Access Link.
///
/// # Errors
/// 403 `permission_denied`, `sudo_required`, `administrator_only` or
/// `stronger_user`; 404 `user_not_found`; 409 `last_administrator`.
#[utoipa::path(
    delete,
    path = "/admin/users/{id}/passkeys",
    operation_id = "removeUserPasskeys",
    tag = "users",
    params(
        ("id" = Uuid, Path, description = "User id"),
        ("x-csrf-token" = String, Header, description = "The CSRF token from sign-in"),
    ),
    security(("session" = [])),
    responses(
        (status = 204, description = "The User has no Passkeys any more"),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "The Role lacks user.manage (code permission_denied), Sudo Mode is closed (code sudo_required), or the User is out of reach (codes administrator_only, stronger_user)", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such User (code user_not_found)", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Passkeys are the only way left for the last Administrator who can sign in (code last_administrator)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn remove_user_passkeys(
    State(state): State<AppState>,
    session: AuthSession,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<StatusCode, ApiError> {
    require_write(&state, &session)?;
    let id = user_id(id)?;
    users::remove_passkeys(&state.db, &session.user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_users, create_user))
        .routes(routes!(update_user))
        .routes(routes!(deactivate_user))
        .routes(routes!(reactivate_user))
        .routes(routes!(issue_access_link))
        .routes(routes!(remove_user_passkeys))
}
