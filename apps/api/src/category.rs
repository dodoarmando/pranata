//! Categories: the Village's one list of labels shared by News Posts and Events.

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
    government::Created,
    role::Permission,
};

#[derive(Serialize, ToSchema)]
pub struct Category {
    pub(crate) id: Uuid,
    pub(crate) name: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CategoryFields {
    /// 1–50 characters, unique whatever the case.
    name: String,
}

impl CategoryFields {
    pub(crate) fn name(&self) -> Result<&str, ApiError> {
        let name = self.name.trim();
        if name.is_empty() || name.chars().count() > 50 {
            return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
        }
        Ok(name)
    }
}

/// Managing Categories is publishing's, not writing's.
pub(crate) fn ensure_publisher(session: &Session) -> Result<(), ApiError> {
    if session.holds(Permission::ContentPublish) {
        Ok(())
    } else {
        Err(ApiError::Status(StatusCode::FORBIDDEN))
    }
}

pub(crate) fn unique_name<T>(result: sqlx::Result<T>) -> Result<T, ApiError> {
    match result {
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(ApiError::Status(StatusCode::CONFLICT))
        }
        result => Ok(result?),
    }
}

/// Every Category by name, for the public site and the admin area.
#[utoipa::path(
    get,
    path = "/categories",
    operation_id = "listCategories",
    tag = "content",
    responses((status = OK, description = "Every Category", body = Vec<Category>)),
)]
pub(crate) async fn list(State(pool): State<PgPool>) -> Result<Json<Vec<Category>>, ApiError> {
    let categories = sqlx::query_as!(
        Category,
        "SELECT id, name FROM categories ORDER BY lower(name)"
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(categories))
}

/// Adds a Category.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `409` for a name in use,
/// `422` for a blank name or one over 50 characters.
#[utoipa::path(
    post,
    path = "/categories",
    operation_id = "createCategory",
    tag = "content",
    request_body = CategoryFields,
    responses(
        (status = CREATED, description = "The new Category", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = CONFLICT, description = "Another Category has the name"),
        (status = UNPROCESSABLE_ENTITY, description = "The name is blank or too long"),
    ),
)]
pub(crate) async fn create(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<CategoryFields>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure_publisher(&session)?;
    let name = body.name()?;
    let mut tx = pool.begin().await?;
    let id = unique_name(
        sqlx::query_scalar!(
            "INSERT INTO categories (name) VALUES ($1) RETURNING id",
            name
        )
        .fetch_one(&mut *tx)
        .await,
    )?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "category.create",
        ("category", Some(&id.to_string())),
        json!([{ "field": "name", "from": null, "to": name }]),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

/// Renames a Category.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// Category, `409` for a name in use, `422` for a blank name or one over 50 characters.
#[utoipa::path(
    put,
    path = "/categories/{id}",
    operation_id = "updateCategory",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Category")),
    request_body = CategoryFields,
    responses(
        (status = NO_CONTENT, description = "The Category is renamed"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Category"),
        (status = CONFLICT, description = "Another Category has the name"),
        (status = UNPROCESSABLE_ENTITY, description = "The name is blank or too long"),
    ),
)]
pub(crate) async fn update(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<CategoryFields>,
) -> Result<StatusCode, ApiError> {
    ensure_publisher(&session)?;
    let name = body.name()?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query_scalar!("SELECT name FROM categories WHERE id = $1 FOR UPDATE", id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if old != name {
        unique_name(
            sqlx::query!("UPDATE categories SET name = $1 WHERE id = $2", name, id)
                .execute(&mut *tx)
                .await,
        )?;
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "category.update",
            ("category", Some(&id.to_string())),
            json!([{ "field": "name", "from": old, "to": name }]),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a Category nothing is filed under.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// Category, `409` while anything is filed under it.
#[utoipa::path(
    delete,
    path = "/categories/{id}",
    operation_id = "deleteCategory",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Category")),
    responses(
        (status = NO_CONTENT, description = "The Category is deleted"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Category"),
        (status = CONFLICT, description = "Something is filed under the Category"),
    ),
)]
pub(crate) async fn delete(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    ensure_publisher(&session)?;
    let mut tx = pool.begin().await?;
    // Whatever is filed under a Category references it, so the delete itself refuses.
    let name = match sqlx::query_scalar!("DELETE FROM categories WHERE id = $1 RETURNING name", id)
        .fetch_optional(&mut *tx)
        .await
    {
        Err(sqlx::Error::Database(error)) if error.is_foreign_key_violation() => {
            return Err(ApiError::Status(StatusCode::CONFLICT));
        }
        name => name?.ok_or(ApiError::Status(StatusCode::NOT_FOUND))?,
    };
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "category.delete",
        ("category", Some(&id.to_string())),
        json!([{ "field": "name", "from": name, "to": null }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
