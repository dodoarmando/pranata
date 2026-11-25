//! Role administration routes. Listing needs `role.manage` or `user.manage`
//! (the Users screen picks Roles from it); every write needs `role.manage` and
//! Sudo Mode. The escalation, validation and domain rules live in
//! [`crate::access::roles`].

use axum::{
    Json,
    extract::{Path, State, rejection::PathRejection},
    http::StatusCode,
};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{
    ApiError, AppState,
    access::{
        Permission,
        roles::{self, RoleDetail, RoleRequest},
    },
    auth::{CurrentUser, session::AuthSession},
    error::Problem,
    http::json::ApiJson,
    ids::RoleId,
};

/// A malformed id names no Role.
fn role_id(path: Result<Path<Uuid>, PathRejection>) -> Result<RoleId, ApiError> {
    path.map(|Path(id)| RoleId(id))
        .map_err(|_| ApiError::NotFound("role_not_found"))
}

/// Check `role.manage` and Sudo Mode, in that order.
fn require_write(state: &AppState, session: &AuthSession) -> Result<(), ApiError> {
    session.user.require(Permission::RoleManage)?;
    session.require_sudo(state.clock.now())
}

/// Every Role: the Administrator Role first, then the others by name.
///
/// # Errors
/// 403 `permission_denied` without `role.manage` or `user.manage`.
#[utoipa::path(
    get,
    path = "/admin/roles",
    operation_id = "listRoles",
    tag = "roles",
    security(("session" = [])),
    responses(
        (status = 200, description = "Every Role", body = Vec<RoleDetail>),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "The Role lacks both role.manage and user.manage (code permission_denied)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn list_roles(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<RoleDetail>>, ApiError> {
    user.require_any(&[Permission::RoleManage, Permission::UserManage])?;
    Ok(Json(roles::list(&state.db).await?))
}

/// Create a Role from the catalogue of Permissions.
///
/// # Errors
/// 403 `permission_denied`, `sudo_required` or `permission_not_held`; 422
/// `validation_failed`; 409 `role_name_taken`.
#[utoipa::path(
    post,
    path = "/admin/roles",
    operation_id = "createRole",
    tag = "roles",
    params(("x-csrf-token" = String, Header, description = "The CSRF token from sign-in")),
    security(("session" = [])),
    request_body = RoleRequest,
    responses(
        (status = 201, description = "The new Role", body = RoleDetail),
        (status = 400, description = "Body is not valid JSON or names a Permission outside the catalogue (code invalid_json)", body = Problem, content_type = "application/problem+json"),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "The Role lacks role.manage (code permission_denied), Sudo Mode is closed (code sudo_required), or the Role would grant a Permission this User lacks (code permission_not_held)", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "Another Role has the name, ignoring case (code role_name_taken)", body = Problem, content_type = "application/problem+json"),
        (status = 415, description = "Request is not marked as JSON (code json_required)", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid fields (code validation_failed; field code name_length)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn create_role(
    State(state): State<AppState>,
    session: AuthSession,
    body: Result<ApiJson<RoleRequest>, ApiError>,
) -> Result<(StatusCode, Json<RoleDetail>), ApiError> {
    require_write(&state, &session)?;
    let ApiJson(request) = body?;
    let role = roles::create(&state.db, &session.user, &request).await?;
    Ok((StatusCode::CREATED, Json(role)))
}

/// Rename a Role or replace its Permissions. Its holders get the new
/// Permissions on their next request.
///
/// # Errors
/// 403 `permission_denied`, `sudo_required`, `own_role`,
/// `administrator_only` or `permission_not_held`; 422 `validation_failed`;
/// 404 `role_not_found`; 409 `role_builtin` or `role_name_taken`.
#[utoipa::path(
    put,
    path = "/admin/roles/{id}",
    operation_id = "updateRole",
    tag = "roles",
    params(
        ("id" = Uuid, Path, description = "Role id"),
        ("x-csrf-token" = String, Header, description = "The CSRF token from sign-in"),
    ),
    security(("session" = [])),
    request_body = RoleRequest,
    responses(
        (status = 200, description = "The edited Role", body = RoleDetail),
        (status = 400, description = "Body is not valid JSON or names a Permission outside the catalogue (code invalid_json)", body = Problem, content_type = "application/problem+json"),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "The Role lacks role.manage (code permission_denied), Sudo Mode is closed (code sudo_required), it is the Role this User holds (code own_role), or the Role grants or would grant a Permission this User lacks (codes administrator_only, permission_not_held)", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such Role (code role_not_found)", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Administrator Role cannot change (code role_builtin), or another Role has the name, ignoring case (code role_name_taken)", body = Problem, content_type = "application/problem+json"),
        (status = 415, description = "Request is not marked as JSON (code json_required)", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid fields (code validation_failed; field code name_length)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn update_role(
    State(state): State<AppState>,
    session: AuthSession,
    id: Result<Path<Uuid>, PathRejection>,
    body: Result<ApiJson<RoleRequest>, ApiError>,
) -> Result<Json<RoleDetail>, ApiError> {
    require_write(&state, &session)?;
    let id = role_id(id)?;
    let ApiJson(request) = body?;
    Ok(Json(
        roles::update(&state.db, &session.user, id, &request).await?,
    ))
}

/// Delete a Role that no User holds, active or not.
///
/// # Errors
/// 403 `permission_denied`, `sudo_required`, `own_role`,
/// `administrator_only` or `permission_not_held`; 404 `role_not_found`; 409
/// `role_builtin` or `role_in_use`.
#[utoipa::path(
    delete,
    path = "/admin/roles/{id}",
    operation_id = "deleteRole",
    tag = "roles",
    params(
        ("id" = Uuid, Path, description = "Role id"),
        ("x-csrf-token" = String, Header, description = "The CSRF token from sign-in"),
    ),
    security(("session" = [])),
    responses(
        (status = 204, description = "The Role is deleted"),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "The Role lacks role.manage (code permission_denied), Sudo Mode is closed (code sudo_required), it is the Role this User holds (code own_role), or the Role grants a Permission this User lacks (codes administrator_only, permission_not_held)", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such Role (code role_not_found)", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The Administrator Role cannot be deleted (code role_builtin), or a User holds the Role, active or not (code role_in_use)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn delete_role(
    State(state): State<AppState>,
    session: AuthSession,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<StatusCode, ApiError> {
    require_write(&state, &session)?;
    let id = role_id(id)?;
    roles::delete(&state.db, &session.user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_roles, create_role))
        .routes(routes!(update_role, delete_role))
}
