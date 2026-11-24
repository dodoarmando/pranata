use axum::{Json, extract::State};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    ApiError, AppState,
    auth::CurrentUser,
    error::Problem,
    village::{
        self,
        profile::{self, Profile, ProfileUpdate},
    },
};

/// The Village profile shown on the public site.
///
/// # Errors
/// 404 `village_not_initialized` before `pranata village init` has run.
#[utoipa::path(
    get,
    path = "/public/village",
    operation_id = "getPublicVillage",
    tag = "public",
    responses(
        (status = 200, description = "The Village profile", body = village::VillageProfile),
        (status = 404, description = "The Village is not initialised yet (code village_not_initialized)", body = crate::error::Problem, content_type = "application/problem+json"),
    )
)]
pub async fn get_public_village(
    State(state): State<AppState>,
) -> Result<Json<village::VillageProfile>, ApiError> {
    village::get(&state.db)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("village_not_initialized"))
}

/// The editable part of the Village profile.
///
/// # Errors
/// 404 `village_not_initialized` before `pranata village init` has run.
#[utoipa::path(
    get,
    path = "/admin/village/profile",
    operation_id = "getVillageProfile",
    tag = "village",
    security(("session" = [])),
    responses(
        (status = 200, description = "The overview and hero photo", body = Profile),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "The Village is not initialised yet (code village_not_initialized)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn get_village_profile(
    State(state): State<AppState>,
    _user: CurrentUser,
) -> Result<Json<Profile>, ApiError> {
    profile::get(&state.db).await.map(Json)
}

/// Replace the overview and hero photo, then purge the public cache.
///
/// # Errors
/// 409 `stale_write`; 422 `validation_failed` (field `overview`) or
/// `hero_not_ready`; 404 `village_not_initialized`.
#[utoipa::path(
    put,
    path = "/admin/village/profile",
    operation_id = "putVillageProfile",
    tag = "village",
    security(("session" = [])),
    request_body = ProfileUpdate,
    responses(
        (status = 200, description = "The saved profile", body = Profile),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "The Village is not initialised yet (code village_not_initialized)", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The profile changed since it was read (code stale_write)", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Invalid overview (code validation_failed, with field errors) or a hero that is not a ready image (code hero_not_ready)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn put_village_profile(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(input): Json<ProfileUpdate>,
) -> Result<Json<Profile>, ApiError> {
    profile::update(&state.db, user.id, input).await.map(Json)
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_public_village))
        .routes(routes!(get_village_profile, put_village_profile))
}
