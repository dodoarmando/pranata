//! Indeks Desa Editions: the ministry's score and status for one data-collection year.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    ApiError,
    audit::{self, Actor},
    auth::Session,
    government::{Created, ensure_manager},
    village::diff,
};

/// Which index scored the Village.
#[derive(Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "index_kind", rename_all = "snake_case")]
pub enum IndexKind {
    /// Indeks Desa, since 2024: 0–100 with 2 decimals and six dimensions.
    IndeksDesa,
    /// Indeks Desa Membangun, before 2024: 0–1 with 4 decimals.
    Idm,
}

/// The status the ministry gave, as entered.
#[derive(Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "development_status", rename_all = "snake_case")]
pub enum DevelopmentStatus {
    /// Mandiri.
    Independent,
    /// Maju.
    Advanced,
    /// Berkembang.
    Developing,
    /// Tertinggal.
    Underdeveloped,
    /// Sangat Tertinggal.
    VeryUnderdeveloped,
}

/// What a User records about an Edition.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct IndexEditionFields {
    /// The data-collection year, 2000–2100; one Edition per year.
    year: i16,
    kind: IndexKind,
    /// 0–100 with 2 decimals for Indeks Desa, 0–1 with 4 decimals for IDM.
    score: f64,
    status: DevelopmentStatus,
    /// Layanan Dasar; this and the other dimensions are 0–100 with 2 decimals, Indeks Desa only.
    #[serde(default)]
    basic_services: Option<f64>,
    /// Sosial.
    #[serde(default)]
    social: Option<f64>,
    /// Ekonomi.
    #[serde(default)]
    economic: Option<f64>,
    /// Lingkungan.
    #[serde(default)]
    environmental: Option<f64>,
    /// Aksesibilitas.
    #[serde(default)]
    accessibility: Option<f64>,
    /// Tata Kelola Pemerintahan.
    #[serde(default)]
    governance: Option<f64>,
}

/// Whether `value` lies in `0..=max` with at most `decimals` decimals.
#[expect(clippy::float_cmp, reason = "a decimal is exactly one double")]
fn in_scale(value: f64, max: f64, decimals: i32) -> bool {
    let scale = 10f64.powi(decimals);
    // Division is correctly rounded, so this is exactly the double the decimal text parses to.
    (0.0..=max).contains(&value) && (value * scale).round() / scale == value
}

impl IndexEditionFields {
    fn dimensions(&self) -> [Option<f64>; 6] {
        [
            self.basic_services,
            self.social,
            self.economic,
            self.environmental,
            self.accessibility,
            self.governance,
        ]
    }

    /// Refuses with `422` a year, score or dimension outside its kind's scale.
    fn validated(self) -> Result<Self, ApiError> {
        let valid = (2000..=2100).contains(&self.year)
            && match self.kind {
                IndexKind::IndeksDesa => {
                    in_scale(self.score, 100.0, 2)
                        && self
                            .dimensions()
                            .into_iter()
                            .flatten()
                            .all(|d| in_scale(d, 100.0, 2))
                }
                IndexKind::Idm => {
                    in_scale(self.score, 1.0, 4) && self.dimensions().iter().all(Option::is_none)
                }
            };
        if valid {
            Ok(self)
        } else {
            Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct IndexEdition {
    id: Uuid,
    #[serde(flatten)]
    fields: IndexEditionFields,
}

/// A second Edition for a year is a `409`.
fn unique_year<T>(result: sqlx::Result<T>) -> Result<T, ApiError> {
    match result {
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(ApiError::Status(StatusCode::CONFLICT))
        }
        result => Ok(result?),
    }
}

/// Every Edition, the newest year first, for the public site and the admin area.
#[utoipa::path(
    get,
    path = "/index-editions",
    operation_id = "listIndexEditions",
    tag = "index-edition",
    responses((status = OK, description = "Every Edition", body = Vec<IndexEdition>)),
)]
pub(crate) async fn list(State(pool): State<PgPool>) -> Result<Json<Vec<IndexEdition>>, ApiError> {
    let editions = sqlx::query!(
        r#"SELECT id, year, kind AS "kind: IndexKind", score::float8 AS "score!",
           status AS "status: DevelopmentStatus", basic_services::float8 AS basic_services,
           social::float8 AS social, economic::float8 AS economic,
           environmental::float8 AS environmental, accessibility::float8 AS accessibility,
           governance::float8 AS governance
           FROM index_editions ORDER BY year DESC"#
    )
    .fetch_all(&pool)
    .await?
    .into_iter()
    .map(|row| IndexEdition {
        id: row.id,
        fields: IndexEditionFields {
            year: row.year,
            kind: row.kind,
            score: row.score,
            status: row.status,
            basic_services: row.basic_services,
            social: row.social,
            economic: row.economic,
            environmental: row.environmental,
            accessibility: row.accessibility,
            governance: row.governance,
        },
    })
    .collect();
    Ok(Json(editions))
}

/// Records an Edition.
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `409` when the year has an
/// Edition, `422` for a year, score or dimension outside its kind's scale, or dimensions on IDM.
#[utoipa::path(
    post,
    path = "/index-editions",
    operation_id = "createIndexEdition",
    tag = "index-edition",
    request_body = IndexEditionFields,
    responses(
        (status = CREATED, description = "The new Edition", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = CONFLICT, description = "The year has an Edition"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is outside its scale"),
    ),
)]
pub(crate) async fn create(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<IndexEditionFields>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure_manager(&session)?;
    let new = body.validated()?;
    let mut tx = pool.begin().await?;
    let id = unique_year(
        sqlx::query_scalar!(
            "INSERT INTO index_editions (year, kind, score, status, basic_services, social,
             economic, environmental, accessibility, governance)
             VALUES ($1, $2, $3::float8, $4, $5::float8, $6::float8, $7::float8, $8::float8,
             $9::float8, $10::float8) RETURNING id",
            new.year,
            new.kind as IndexKind,
            new.score,
            new.status as DevelopmentStatus,
            new.basic_services,
            new.social,
            new.economic,
            new.environmental,
            new.accessibility,
            new.governance,
        )
        .fetch_one(&mut *tx)
        .await,
    )?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "index_edition.create",
        ("index_edition", Some(&id.to_string())),
        diff(&Value::Null, &serde_json::to_value(&new)?).into(),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

/// Changes an Edition.
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `404` for an unknown Edition,
/// `409` when the year has another Edition, `422` for a year, score or dimension outside its
/// kind's scale, or dimensions on IDM.
#[utoipa::path(
    put,
    path = "/index-editions/{id}",
    operation_id = "updateIndexEdition",
    tag = "index-edition",
    params(("id" = Uuid, Path, description = "The Edition")),
    request_body = IndexEditionFields,
    responses(
        (status = NO_CONTENT, description = "The Edition is changed"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = NOT_FOUND, description = "There is no such Edition"),
        (status = CONFLICT, description = "The year has another Edition"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is outside its scale"),
    ),
)]
pub(crate) async fn update(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<IndexEditionFields>,
) -> Result<StatusCode, ApiError> {
    ensure_manager(&session)?;
    let new = body.validated()?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query_as!(
        IndexEditionFields,
        r#"SELECT year, kind AS "kind: IndexKind", score::float8 AS "score!",
           status AS "status: DevelopmentStatus", basic_services::float8 AS basic_services,
           social::float8 AS social, economic::float8 AS economic,
           environmental::float8 AS environmental, accessibility::float8 AS accessibility,
           governance::float8 AS governance
           FROM index_editions WHERE id = $1 FOR UPDATE"#,
        id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    unique_year(
        sqlx::query!(
            "UPDATE index_editions SET year = $1, kind = $2, score = $3::float8, status = $4,
             basic_services = $5::float8, social = $6::float8, economic = $7::float8,
             environmental = $8::float8, accessibility = $9::float8, governance = $10::float8
             WHERE id = $11",
            new.year,
            new.kind as IndexKind,
            new.score,
            new.status as DevelopmentStatus,
            new.basic_services,
            new.social,
            new.economic,
            new.environmental,
            new.accessibility,
            new.governance,
            id,
        )
        .execute(&mut *tx)
        .await,
    )?;
    let changes = diff(&serde_json::to_value(&old)?, &serde_json::to_value(&new)?);
    if !changes.is_empty() {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "index_edition.update",
            ("index_edition", Some(&id.to_string())),
            changes.into(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes an Edition.
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `404` for an unknown Edition.
#[utoipa::path(
    delete,
    path = "/index-editions/{id}",
    operation_id = "deleteIndexEdition",
    tag = "index-edition",
    params(("id" = Uuid, Path, description = "The Edition")),
    responses(
        (status = NO_CONTENT, description = "The Edition is deleted"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = NOT_FOUND, description = "There is no such Edition"),
    ),
)]
pub(crate) async fn delete(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    ensure_manager(&session)?;
    let mut tx = pool.begin().await?;
    let year = sqlx::query_scalar!(
        "DELETE FROM index_editions WHERE id = $1 RETURNING year",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "index_edition.delete",
        ("index_edition", Some(&id.to_string())),
        json!([{ "field": "year", "from": year, "to": null }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
