//! APBDes Summaries: one fiscal year's village budget and, optionally, its realisation,
//! entered by hand in whole rupiah, saved as a Draft and then Published on `/apbdes` beside
//! the same year's Published budget Transparency Documents.
//!
//! `content.write` creates and edits Drafts; `content.publish` also publishes, unpublishes,
//! edits Published summaries and deletes Drafts.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{
    ApiError, AppState,
    audit::{self, Actor},
    auth::Session,
    category::ensure_publisher,
    government::Created,
    news_post::{ensure_writer, saved},
    transparency_document::{PublishedTransparencyDocument, budget_documents},
    village::diff,
};

pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(update, delete))
        .routes(routes!(publish))
        .routes(routes!(unpublish))
        .routes(routes!(get_published))
}

/// Which APBDes the figures follow.
#[derive(Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[sqlx(type_name = "budget_version", rename_all = "snake_case")]
pub enum BudgetVersion {
    /// The budget as first enacted, `Murni`.
    Original,
    /// The amended budget, `Perubahan`.
    Amendment,
}

/// The most a line holds, so that every figure stays exact in a browser's numbers.
const MAX_RUPIAH: i64 = 999_999_999_999_999;

/// One set of APBDes lines in whole rupiah, each 0 to 999 999 999 999 999.
#[derive(Serialize, Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct BudgetLines {
    /// Pendapatan.
    revenue: i64,
    /// Belanja, Bidang Penyelenggaraan Pemerintahan Desa.
    governance: i64,
    /// Belanja, Bidang Pelaksanaan Pembangunan Desa.
    development: i64,
    /// Belanja, Bidang Pembinaan Kemasyarakatan Desa.
    community: i64,
    /// Belanja, Bidang Pemberdayaan Masyarakat Desa.
    empowerment: i64,
    /// Belanja, Bidang Penanggulangan Bencana, Keadaan Darurat dan Mendesak Desa.
    emergencies: i64,
    /// Penerimaan pembiayaan.
    financing_receipts: i64,
    /// Pengeluaran pembiayaan.
    financing_expenditures: i64,
}

impl BudgetLines {
    fn valid(&self) -> bool {
        [
            self.revenue,
            self.governance,
            self.development,
            self.community,
            self.empowerment,
            self.emergencies,
            self.financing_receipts,
            self.financing_expenditures,
        ]
        .iter()
        .all(|line| (0..=MAX_RUPIAH).contains(line))
    }
}

/// The realised lines so far.
#[derive(Serialize, Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct Realisation {
    /// The day the realisation is counted to.
    as_of: NaiveDate,
    #[serde(flatten)]
    lines: BudgetLines,
}

/// What a User writes in an APBDes Summary.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApbdesSummaryFields {
    /// 2000–2100, at most one summary each.
    fiscal_year: i32,
    version: BudgetVersion,
    /// The day this version was enacted.
    version_date: NaiveDate,
    budget: BudgetLines,
    /// Null before any realisation is reported.
    realisation: Option<Realisation>,
}

impl ApbdesSummaryFields {
    /// Refuses with `422` what is out of range.
    fn validated(self) -> Result<Self, ApiError> {
        if (2000..=2100).contains(&self.fiscal_year)
            && self.budget.valid()
            && self
                .realisation
                .is_none_or(|realised| realised.lines.valid())
        {
            Ok(self)
        } else {
            Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))
        }
    }

    fn from_columns(
        fiscal_year: i32,
        version: BudgetVersion,
        version_date: NaiveDate,
        budget: Value,
        realisation: Option<Value>,
    ) -> Result<Self, ApiError> {
        Ok(Self {
            fiscal_year,
            version,
            version_date,
            budget: serde_json::from_value(budget)?,
            realisation: realisation.map(serde_json::from_value).transpose()?,
        })
    }

    /// The budget and realisation as stored in their JSONB columns.
    fn columns(&self) -> Result<(Value, Option<Value>), ApiError> {
        Ok((
            serde_json::to_value(self.budget)?,
            self.realisation.map(serde_json::to_value).transpose()?,
        ))
    }
}

/// An APBDes Summary as the admin area lists and edits it.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApbdesSummary {
    id: Uuid,
    #[serde(flatten)]
    fields: ApbdesSummaryFields,
    published: bool,
}

/// Every APBDes Summary, Drafts and Published, the latest fiscal year first.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`.
#[utoipa::path(
    get,
    path = "/apbdes-summaries",
    operation_id = "listApbdesSummaries",
    tag = "content",
    responses(
        (status = OK, description = "Every APBDes Summary", body = Vec<ApbdesSummary>),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
    ),
)]
pub(crate) async fn list(
    State(pool): State<PgPool>,
    session: Session,
) -> Result<Json<Vec<ApbdesSummary>>, ApiError> {
    ensure_writer(&session)?;
    let rows = sqlx::query!(
        r#"SELECT id, fiscal_year, version AS "version: BudgetVersion", version_date, budget,
           realisation, published FROM apbdes_summaries ORDER BY fiscal_year DESC"#
    )
    .fetch_all(&pool)
    .await?;
    let summaries = rows
        .into_iter()
        .map(|row| {
            Ok(ApbdesSummary {
                id: row.id,
                fields: ApbdesSummaryFields::from_columns(
                    row.fiscal_year,
                    row.version,
                    row.version_date,
                    row.budget,
                    row.realisation,
                )?,
                published: row.published,
            })
        })
        .collect::<Result<_, ApiError>>()?;
    Ok(Json(summaries))
}

/// Saves a new APBDes Summary as a Draft.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `409`
/// when the fiscal year already has a summary, `422` for a fiscal year outside 2000–2100 or
/// a line that is not a whole rupiah amount from 0 to 999 999 999 999 999.
#[utoipa::path(
    post,
    path = "/apbdes-summaries",
    operation_id = "createApbdesSummary",
    tag = "content",
    request_body = ApbdesSummaryFields,
    responses(
        (status = CREATED, description = "The new Draft", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
        (status = CONFLICT, description = "The fiscal year already has a summary"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed"),
    ),
)]
pub(crate) async fn create(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<ApbdesSummaryFields>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure_writer(&session)?;
    let new = body.validated()?;
    let (budget, realisation) = new.columns()?;
    let mut tx = pool.begin().await?;
    let id = saved(
        sqlx::query_scalar!(
            "INSERT INTO apbdes_summaries (fiscal_year, version, version_date, budget, realisation)
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
            new.fiscal_year,
            new.version as BudgetVersion,
            new.version_date,
            budget,
            realisation,
        )
        .fetch_one(&mut *tx)
        .await,
    )?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "apbdes_summary.create",
        ("apbdes_summary", Some(&id.to_string())),
        diff(&Value::Null, &serde_json::to_value(&new)?).into(),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

/// Saves an APBDes Summary, a Draft or a Published one live.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` (or `content.publish`) for a
/// Draft or without `content.publish` for a Published summary, `404` for an unknown summary,
/// `409` when another summary has the fiscal year, `422` for a fiscal year outside 2000–2100
/// or a line that is not a whole rupiah amount from 0 to 999 999 999 999 999.
#[utoipa::path(
    put,
    path = "/apbdes-summaries/{id}",
    operation_id = "updateApbdesSummary",
    tag = "content",
    params(("id" = Uuid, Path, description = "The APBDes Summary")),
    request_body = ApbdesSummaryFields,
    responses(
        (status = NO_CONTENT, description = "The summary is saved"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not change this summary"),
        (status = NOT_FOUND, description = "There is no such summary"),
        (status = CONFLICT, description = "Another summary has the fiscal year"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed"),
    ),
)]
pub(crate) async fn update(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<ApbdesSummaryFields>,
) -> Result<StatusCode, ApiError> {
    ensure_writer(&session)?;
    let new = body.validated()?;
    let (budget, realisation) = new.columns()?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        r#"SELECT fiscal_year, version AS "version: BudgetVersion", version_date, budget,
           realisation, published FROM apbdes_summaries WHERE id = $1 FOR UPDATE"#,
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if old.published {
        ensure_publisher(&session)?;
    }
    saved(
        sqlx::query!(
            "UPDATE apbdes_summaries SET fiscal_year = $1, version = $2, version_date = $3,
             budget = $4, realisation = $5, updated_at = now() WHERE id = $6",
            new.fiscal_year,
            new.version as BudgetVersion,
            new.version_date,
            budget,
            realisation,
            id,
        )
        .execute(&mut *tx)
        .await,
    )?;
    let from = ApbdesSummaryFields::from_columns(
        old.fiscal_year,
        old.version,
        old.version_date,
        old.budget,
        old.realisation,
    )?;
    let changes = diff(&serde_json::to_value(from)?, &serde_json::to_value(&new)?);
    if !changes.is_empty() {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "apbdes_summary.update",
            ("apbdes_summary", Some(&id.to_string())),
            changes.into(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Puts a Draft on `/apbdes`.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// summary, `409` when it is already Published.
#[utoipa::path(
    post,
    path = "/apbdes-summaries/{id}/publish",
    operation_id = "publishApbdesSummary",
    tag = "content",
    params(("id" = Uuid, Path, description = "The APBDes Summary")),
    responses(
        (status = NO_CONTENT, description = "The summary is Published"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such summary"),
        (status = CONFLICT, description = "The summary is already Published"),
    ),
)]
pub(crate) async fn publish(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    set_published(&pool, &session, id, true).await
}

/// Takes a Published APBDes Summary off `/apbdes`, back to Draft.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// summary, `409` when it is a Draft.
#[utoipa::path(
    post,
    path = "/apbdes-summaries/{id}/unpublish",
    operation_id = "unpublishApbdesSummary",
    tag = "content",
    params(("id" = Uuid, Path, description = "The APBDes Summary")),
    responses(
        (status = NO_CONTENT, description = "The summary is a Draft again"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such summary"),
        (status = CONFLICT, description = "The summary is a Draft"),
    ),
)]
pub(crate) async fn unpublish(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    set_published(&pool, &session, id, false).await
}

async fn set_published(
    pool: &PgPool,
    session: &Session,
    id: Uuid,
    published: bool,
) -> Result<StatusCode, ApiError> {
    ensure_publisher(session)?;
    let mut tx = pool.begin().await?;
    let was = sqlx::query_scalar!(
        "SELECT published FROM apbdes_summaries WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if was == published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    sqlx::query!(
        "UPDATE apbdes_summaries SET published = $1 WHERE id = $2",
        published,
        id
    )
    .execute(&mut *tx)
    .await?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        if published {
            "apbdes_summary.publish"
        } else {
            "apbdes_summary.unpublish"
        },
        ("apbdes_summary", Some(&id.to_string())),
        json!([{ "field": "published", "from": was, "to": published }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a Draft for good. A Published summary is unpublished first.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// summary, `409` when it is Published.
#[utoipa::path(
    delete,
    path = "/apbdes-summaries/{id}",
    operation_id = "deleteApbdesSummary",
    tag = "content",
    params(("id" = Uuid, Path, description = "The APBDes Summary")),
    responses(
        (status = NO_CONTENT, description = "The Draft is deleted"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such summary"),
        (status = CONFLICT, description = "The summary is Published"),
    ),
)]
pub(crate) async fn delete(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    ensure_publisher(&session)?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        "SELECT fiscal_year, published FROM apbdes_summaries WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if old.published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    sqlx::query!("DELETE FROM apbdes_summaries WHERE id = $1", id)
        .execute(&mut *tx)
        .await?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "apbdes_summary.delete",
        ("apbdes_summary", Some(&id.to_string())),
        json!([{ "field": "fiscalYear", "from": old.fiscal_year, "to": null }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase")]
pub struct ApbdesSummaryFilter {
    /// The fiscal year; the latest Published one when absent.
    fiscal_year: Option<i32>,
}

/// A Published APBDes Summary on `/apbdes`.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishedApbdesSummary {
    #[serde(flatten)]
    fields: ApbdesSummaryFields,
    /// The year's Published `APBDes`, `APBDes Perubahan` and `Laporan realisasi` documents,
    /// the newest first.
    documents: Vec<PublishedTransparencyDocument>,
    /// Every fiscal year with a Published summary, the latest first, for the year switch.
    fiscal_years: Vec<i32>,
}

/// A fiscal year's Published APBDes Summary, by default the latest, with its budget documents.
///
/// # Errors
/// `404` when that year, or any year, has no Published summary.
#[utoipa::path(
    get,
    path = "/apbdes-summaries/published",
    operation_id = "getPublishedApbdesSummary",
    tag = "content",
    params(ApbdesSummaryFilter),
    responses(
        (status = OK, description = "The Published APBDes Summary", body = PublishedApbdesSummary),
        (status = BAD_REQUEST, description = "The filter is malformed"),
        (status = NOT_FOUND, description = "There is no such Published summary"),
    ),
)]
pub(crate) async fn get_published(
    State(pool): State<PgPool>,
    Query(filter): Query<ApbdesSummaryFilter>,
) -> Result<Json<PublishedApbdesSummary>, ApiError> {
    let row = sqlx::query!(
        r#"SELECT fiscal_year, version AS "version: BudgetVersion", version_date, budget,
           realisation FROM apbdes_summaries
           WHERE published AND ($1::integer IS NULL OR fiscal_year = $1)
           ORDER BY fiscal_year DESC LIMIT 1"#,
        filter.fiscal_year,
    )
    .fetch_optional(&pool)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    let fiscal_years = sqlx::query_scalar!(
        "SELECT fiscal_year FROM apbdes_summaries WHERE published ORDER BY fiscal_year DESC"
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(PublishedApbdesSummary {
        documents: budget_documents(&pool, row.fiscal_year).await?,
        fields: ApbdesSummaryFields::from_columns(
            row.fiscal_year,
            row.version,
            row.version_date,
            row.budget,
            row.realisation,
        )?,
        fiscal_years,
    }))
}
