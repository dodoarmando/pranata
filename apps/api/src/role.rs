use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    ApiError,
    audit::{self, Actor},
    auth::Session,
};

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

#[derive(Deserialize, ToSchema)]
pub struct RoleInput {
    /// Unique among Roles; surrounding spaces are dropped.
    name: String,
    permissions: Vec<Permission>,
}

impl RoleInput {
    /// The trimmed name, refused with `422` when blank.
    fn name(&self) -> Result<&str, ApiError> {
        match self.name.trim() {
            "" => Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY)),
            name => Ok(name),
        }
    }
}

/// Creates a Role from the Permission catalog.
///
/// # Errors
/// `401` without a live session, `403` outside the Administrator Role, `409` for a taken
/// name, `428` outside sudo mode, `422` for a blank name or a Permission outside the catalog.
#[utoipa::path(
    post,
    path = "/roles",
    operation_id = "createRole",
    tag = "roles",
    request_body = RoleInput,
    responses(
        (status = CREATED, description = "The new Role", body = Role),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User is not an Administrator"),
        (status = CONFLICT, description = "Another Role has the name"),
        (status = PRECONDITION_REQUIRED, description = "The User must re-authenticate first"),
        (status = UNPROCESSABLE_ENTITY, description = "The name is blank or a Permission is unknown"),
    ),
)]
pub async fn create(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<RoleInput>,
) -> Result<(StatusCode, Json<Role>), ApiError> {
    session.ensure_administrator()?;
    session.ensure_sudo()?;
    let name = body.name()?;
    let mut tx = pool.begin().await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO roles (name, permissions) VALUES ($1, $2)
         ON CONFLICT (name) DO NOTHING RETURNING id",
        name,
        body.permissions.as_slice() as _,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::CONFLICT))?;
    audit::record(
        &mut tx,
        Actor::User(session.user_id),
        "role.create",
        ("role", Some(&id.to_string())),
        json!({ "name": name, "permissions": body.permissions }),
    )
    .await?;
    tx.commit().await?;
    let role = Role {
        id,
        name: name.to_owned(),
        administrator: false,
        permissions: body.permissions,
    };
    Ok((StatusCode::CREATED, Json(role)))
}

/// Renames a Role and replaces its Permissions; its Users hold them from their next request.
///
/// # Errors
/// `401` without a live session, `403` outside the Administrator Role, `404` for an unknown
/// Role, `409` for the locked Administrator Role or a taken name, `428` outside sudo mode,
/// `422` as for creating.
#[utoipa::path(
    put,
    path = "/roles/{id}",
    operation_id = "updateRole",
    tag = "roles",
    params(("id" = Uuid, Path, description = "The Role")),
    request_body = RoleInput,
    responses(
        (status = NO_CONTENT, description = "The Role is changed"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User is not an Administrator"),
        (status = NOT_FOUND, description = "There is no such Role"),
        (status = CONFLICT, description = "The Role is the Administrator, or another Role has the name"),
        (status = PRECONDITION_REQUIRED, description = "The User must re-authenticate first"),
        (status = UNPROCESSABLE_ENTITY, description = "The name is blank or a Permission is unknown"),
    ),
)]
pub async fn update(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<RoleInput>,
) -> Result<StatusCode, ApiError> {
    session.ensure_administrator()?;
    session.ensure_sudo()?;
    let name = body.name()?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        r#"SELECT name, administrator, permissions AS "permissions: Vec<Permission>"
           FROM roles WHERE id = $1 FOR UPDATE"#,
        id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if old.administrator {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    let updated = sqlx::query!(
        "UPDATE roles SET name = $1, permissions = $2 WHERE id = $3",
        name,
        body.permissions.as_slice() as _,
        id,
    )
    .execute(&mut *tx)
    .await;
    match updated {
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            return Err(ApiError::Status(StatusCode::CONFLICT));
        }
        updated => updated?,
    };
    let mut changes = Vec::new();
    if old.name != name {
        changes.push(json!({ "field": "name", "from": old.name, "to": name }));
    }
    if old.permissions != body.permissions {
        changes.push(
            json!({ "field": "permissions", "from": old.permissions, "to": body.permissions }),
        );
    }
    if !changes.is_empty() {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "role.update",
            ("role", Some(&id.to_string())),
            changes.into(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
