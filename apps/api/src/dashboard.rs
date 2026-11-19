use axum::{Json, extract::State};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::{ApiError, auth::Session};

/// The figures behind the admin dashboard's cards.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    active_users: i64,
}

/// The admin dashboard's figures, for any signed-in User.
///
/// # Errors
/// `401` without a live session.
#[utoipa::path(
    get,
    path = "/dashboard",
    operation_id = "getDashboard",
    tag = "dashboard",
    responses(
        (status = OK, description = "The dashboard's figures", body = Dashboard),
        (status = UNAUTHORIZED, description = "There is no live session"),
    ),
)]
pub async fn get(
    State(pool): State<PgPool>,
    _session: Session,
) -> Result<Json<Dashboard>, ApiError> {
    let active_users =
        sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM users WHERE active"#)
            .fetch_one(&pool)
            .await?;
    Ok(Json(Dashboard { active_users }))
}
