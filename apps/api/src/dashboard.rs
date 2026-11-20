use axum::{Json, extract::State};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::{ApiError, auth::Session, village::TimeZone};

/// The figures behind the admin dashboard's cards.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    active_users: i64,
    missing_village_data: MissingVillageData,
}

/// What the Village has yet to fill in.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MissingVillageData {
    logo: bool,
    vision_mission: bool,
    /// Active Head-of-Hamlet Positions no Official serves in today.
    unfilled_positions: Vec<String>,
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
    let village = sqlx::query!(
        "SELECT code, hamlet_label, logo_file_id IS NULL AS \"logo!\",
         vision IS NULL AS \"vision_mission!\" FROM village"
    )
    .fetch_optional(&pool)
    .await?;
    let unfilled_positions = match &village {
        None => Vec::new(),
        Some(village) => {
            sqlx::query_scalar!(
                r#"SELECT 'Kepala ' || initcap($2) || ' ' || hamlets.name AS "name!"
                   FROM positions JOIN region_units hamlets ON hamlets.id = hamlet_id
                   WHERE positions.active AND NOT EXISTS (SELECT 1 FROM tenures
                       WHERE position_id = positions.id
                       AND start_date <= (now() AT TIME ZONE $1)::date
                       AND (end_date IS NULL OR end_date >= (now() AT TIME ZONE $1)::date))
                   ORDER BY hamlets.sort_order"#,
                TimeZone::of_province(&village.code).iana(),
                village.hamlet_label,
            )
            .fetch_all(&pool)
            .await?
        }
    };
    Ok(Json(Dashboard {
        active_users,
        missing_village_data: MissingVillageData {
            logo: village.as_ref().is_none_or(|village| village.logo),
            vision_mission: village
                .as_ref()
                .is_none_or(|village| village.vision_mission),
            unfilled_positions,
        },
    }))
}
