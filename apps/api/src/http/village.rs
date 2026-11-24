use axum::{Json, extract::State};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{ApiError, AppState, village};

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

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(get_public_village))
}
