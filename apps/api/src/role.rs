use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{ApiError, auth::Session};

/// The fixed catalog of Permissions a Role can hold. Managing Users and Roles is not one:
/// it belongs to the Administrator Role alone.
#[derive(Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, Debug, PartialEq, Eq)]
#[sqlx(type_name = "text")]
pub enum Permission {
    #[serde(rename = "village.manage")]
    #[sqlx(rename = "village.manage")]
    VillageManage,
    #[serde(rename = "content.write")]
    #[sqlx(rename = "content.write")]
    ContentWrite,
    #[serde(rename = "content.publish")]
    #[sqlx(rename = "content.publish")]
    ContentPublish,
    #[serde(rename = "complaint.read")]
    #[sqlx(rename = "complaint.read")]
    ComplaintRead,
    #[serde(rename = "complaint.handle")]
    #[sqlx(rename = "complaint.handle")]
    ComplaintHandle,
    #[serde(rename = "audit.read")]
    #[sqlx(rename = "audit.read")]
    AuditRead,
}

#[derive(Serialize, ToSchema)]
pub struct Role {
    id: Uuid,
    name: String,
    /// The built-in, locked Administrator Role, which holds every Permission whatever `permissions` says.
    administrator: bool,
    permissions: Vec<Permission>,
}

/// Every Role, the Administrator first.
///
/// # Errors
/// `401` without a live session, `403` outside the Administrator Role.
#[utoipa::path(
    get,
    path = "/roles",
    operation_id = "listRoles",
    tag = "roles",
    responses(
        (status = OK, description = "Every Role", body = Vec<Role>),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User is not an Administrator"),
    ),
)]
pub async fn list(
    State(pool): State<PgPool>,
    session: Session,
) -> Result<Json<Vec<Role>>, ApiError> {
    session.ensure_administrator()?;
    let roles = sqlx::query_as!(
        Role,
        r#"SELECT id, name, administrator, permissions AS "permissions: Vec<Permission>"
           FROM roles ORDER BY administrator DESC, id"#,
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(roles))
}
