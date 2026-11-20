//! Transparency Documents: official planning, budget and regulation PDFs, saved as Drafts and
//! then Published on `/dokumen` for anyone to download, by type group and fiscal year.
//!
//! `content.write` creates and edits Drafts and uploads PDFs; `content.publish` also
//! publishes, unpublishes, edits Published documents and deletes Drafts.

use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
};
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
    storage::{self, Storage},
    village::{Upload, diff},
};

pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(update, delete))
        .routes(routes!(publish))
        .routes(routes!(unpublish))
        .routes(routes!(upload_file))
        .routes(routes!(list_published))
}

/// A Transparency Document's type, from a fixed catalog.
#[derive(Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[sqlx(type_name = "document_type", rename_all = "snake_case")]
pub enum TransparencyDocumentType {
    /// The village budget, `APBDes`.
    Apbdes,
    /// Its amendment, `APBDes Perubahan`.
    ApbdesAmendment,
    /// The realisation report, `Laporan realisasi`.
    RealisationReport,
    /// The annual work plan, `RKPDes`.
    Rkpdes,
    /// The medium-term development plan, `RPJMDes`.
    Rpjmdes,
    /// A village regulation, `Perdes`.
    VillageRegulation,
    /// Anything else, `Lainnya`, listed under no group.
    Other,
}

/// A public filter over document types.
#[derive(Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum TransparencyDocumentGroup {
    /// `APBDes` and its amendment.
    Budget,
    /// Realisation reports.
    Realisation,
    /// Plans, `RKPDes` and `RPJMDes`.
    Planning,
    /// Village regulations.
    Regulation,
}

impl TransparencyDocumentGroup {
    fn types(self) -> &'static [TransparencyDocumentType] {
        match self {
            Self::Budget => &[
                TransparencyDocumentType::Apbdes,
                TransparencyDocumentType::ApbdesAmendment,
            ],
            Self::Realisation => &[TransparencyDocumentType::RealisationReport],
            Self::Planning => &[
                TransparencyDocumentType::Rkpdes,
                TransparencyDocumentType::Rpjmdes,
            ],
            Self::Regulation => &[TransparencyDocumentType::VillageRegulation],
        }
    }
}

/// What a User writes in a Transparency Document.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyDocumentFields {
    /// 1–200 characters.
    title: String,
    document_type: TransparencyDocumentType,
    /// 2000–2100.
    fiscal_year: i32,
    /// The PDF, uploaded at `/transparency-documents/file`.
    file: Uuid,
}

impl TransparencyDocumentFields {
    /// Trims the title and refuses with `422` what is malformed.
    fn normalized(self) -> Result<Self, ApiError> {
        let title = self.title.trim();
        if title.is_empty()
            || title.chars().count() > 200
            || !(2000..=2100).contains(&self.fiscal_year)
        {
            return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
        }
        Ok(Self {
            title: title.to_owned(),
            ..self
        })
    }

    fn audited(&self) -> Value {
        json!({
            "title": self.title, "documentType": self.document_type,
            "fiscalYear": self.fiscal_year, "file": self.file,
        })
    }
}

/// `422` unless `file` is a stored PDF rather than an image or nothing.
async fn ensure_pdf(conn: &mut sqlx::PgConnection, file: Uuid) -> Result<(), ApiError> {
    sqlx::query_scalar!(
        "SELECT 1 AS one FROM files WHERE id = $1 AND byte_size IS NOT NULL",
        file
    )
    .fetch_optional(conn)
    .await?
    .map(|_| ())
    .ok_or(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))
}

/// A Transparency Document as the admin area lists and edits it.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyDocument {
    id: Uuid,
    #[serde(flatten)]
    fields: TransparencyDocumentFields,
    /// The PDF's size.
    byte_size: i64,
    published: bool,
}

/// Every Transparency Document, Drafts and Published, the latest fiscal year first.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`.
#[utoipa::path(
    get,
    path = "/transparency-documents",
    operation_id = "listTransparencyDocuments",
    tag = "content",
    responses(
        (status = OK, description = "Every Transparency Document", body = Vec<TransparencyDocument>),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
    ),
)]
pub(crate) async fn list(
    State(pool): State<PgPool>,
    session: Session,
) -> Result<Json<Vec<TransparencyDocument>>, ApiError> {
    ensure_writer(&session)?;
    let rows = sqlx::query!(
        r#"SELECT documents.id, title, document_type AS "document_type: TransparencyDocumentType",
           fiscal_year, file_id, byte_size AS "byte_size!", published
           FROM transparency_documents documents JOIN files ON files.id = file_id
           ORDER BY fiscal_year DESC, documents.id DESC"#
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| TransparencyDocument {
                id: row.id,
                fields: TransparencyDocumentFields {
                    title: row.title,
                    document_type: row.document_type,
                    fiscal_year: row.fiscal_year,
                    file: row.file_id,
                },
                byte_size: row.byte_size,
                published: row.published,
            })
            .collect(),
    ))
}

/// Saves a new Transparency Document as a Draft.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `422`
/// for a blank title, a type outside the catalog, a fiscal year outside 2000–2100 or a file
/// that is not an uploaded PDF.
#[utoipa::path(
    post,
    path = "/transparency-documents",
    operation_id = "createTransparencyDocument",
    tag = "content",
    request_body = TransparencyDocumentFields,
    responses(
        (status = CREATED, description = "The new Draft", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed or unknown"),
    ),
)]
pub(crate) async fn create(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<TransparencyDocumentFields>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure_writer(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    ensure_pdf(&mut tx, new.file).await?;
    let id = saved(
        sqlx::query_scalar!(
            "INSERT INTO transparency_documents (title, document_type, fiscal_year, file_id)
             VALUES ($1, $2, $3, $4) RETURNING id",
            new.title,
            new.document_type as TransparencyDocumentType,
            new.fiscal_year,
            new.file,
        )
        .fetch_one(&mut *tx)
        .await,
    )?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "transparency_document.create",
        ("transparency_document", Some(&id.to_string())),
        diff(&Value::Null, &new.audited()).into(),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

/// Saves a Transparency Document, a Draft or a Published one live.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` (or `content.publish`) for a
/// Draft or without `content.publish` for a Published document, `404` for an unknown
/// document, `422` for a blank title, a type outside the catalog, a fiscal year outside
/// 2000–2100 or a file that is not an uploaded PDF.
#[utoipa::path(
    put,
    path = "/transparency-documents/{id}",
    operation_id = "updateTransparencyDocument",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Transparency Document")),
    request_body = TransparencyDocumentFields,
    responses(
        (status = NO_CONTENT, description = "The document is saved"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not change this document"),
        (status = NOT_FOUND, description = "There is no such document"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed or unknown"),
    ),
)]
pub(crate) async fn update(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<TransparencyDocumentFields>,
) -> Result<StatusCode, ApiError> {
    ensure_writer(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        r#"SELECT title, document_type AS "document_type: TransparencyDocumentType", fiscal_year, file_id,
           published FROM transparency_documents WHERE id = $1 FOR UPDATE"#,
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if old.published {
        ensure_publisher(&session)?;
    }
    ensure_pdf(&mut tx, new.file).await?;
    saved(
        sqlx::query!(
            "UPDATE transparency_documents SET title = $1, document_type = $2, fiscal_year = $3,
             file_id = $4, updated_at = now() WHERE id = $5",
            new.title,
            new.document_type as TransparencyDocumentType,
            new.fiscal_year,
            new.file,
            id,
        )
        .execute(&mut *tx)
        .await,
    )?;
    if old.file_id != new.file {
        storage::release(&mut tx, old.file_id).await?;
    }
    let from = TransparencyDocumentFields {
        title: old.title,
        document_type: old.document_type,
        fiscal_year: old.fiscal_year,
        file: old.file_id,
    }
    .audited();
    let changes = diff(&from, &new.audited());
    if !changes.is_empty() {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "transparency_document.update",
            ("transparency_document", Some(&id.to_string())),
            changes.into(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Puts a Draft on `/dokumen`.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// document, `409` when it is already Published.
#[utoipa::path(
    post,
    path = "/transparency-documents/{id}/publish",
    operation_id = "publishTransparencyDocument",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Transparency Document")),
    responses(
        (status = NO_CONTENT, description = "The document is Published"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such document"),
        (status = CONFLICT, description = "The document is already Published"),
    ),
)]
pub(crate) async fn publish(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    set_published(&pool, &session, id, true).await
}

/// Takes a Published Transparency Document off `/dokumen`, back to Draft.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// document, `409` when it is a Draft.
#[utoipa::path(
    post,
    path = "/transparency-documents/{id}/unpublish",
    operation_id = "unpublishTransparencyDocument",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Transparency Document")),
    responses(
        (status = NO_CONTENT, description = "The document is a Draft again"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such document"),
        (status = CONFLICT, description = "The document is a Draft"),
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
        "SELECT published FROM transparency_documents WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if was == published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    sqlx::query!(
        "UPDATE transparency_documents SET published = $1 WHERE id = $2",
        published,
        id
    )
    .execute(&mut *tx)
    .await?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        if published {
            "transparency_document.publish"
        } else {
            "transparency_document.unpublish"
        },
        ("transparency_document", Some(&id.to_string())),
        json!([{ "field": "published", "from": was, "to": published }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a Draft for good. A Published document is unpublished first.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// document, `409` when it is Published.
#[utoipa::path(
    delete,
    path = "/transparency-documents/{id}",
    operation_id = "deleteTransparencyDocument",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Transparency Document")),
    responses(
        (status = NO_CONTENT, description = "The Draft is deleted"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such document"),
        (status = CONFLICT, description = "The document is Published"),
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
        "SELECT title, file_id, published FROM transparency_documents WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if old.published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    sqlx::query!("DELETE FROM transparency_documents WHERE id = $1", id)
        .execute(&mut *tx)
        .await?;
    storage::release(&mut tx, old.file_id).await?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "transparency_document.delete",
        ("transparency_document", Some(&id.to_string())),
        json!([{ "field": "title", "from": old.title, "to": null }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The largest PDF a Transparency Document takes.
const MAX_PDF_BYTES: usize = 25 << 20;

/// Stores a PDF of at most 25 MiB, for saving into a Transparency Document.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `413`
/// over 25 MiB, `415` for content that is not a PDF.
#[utoipa::path(
    post,
    path = "/transparency-documents/file",
    operation_id = "uploadTransparencyDocumentFile",
    tag = "content",
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = CREATED, description = "The stored PDF", body = Upload),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
        (status = PAYLOAD_TOO_LARGE, description = "The file is over 25 MiB"),
        (status = UNSUPPORTED_MEDIA_TYPE, description = "The file is not a PDF"),
    ),
)]
pub(crate) async fn upload_file(
    State(pool): State<PgPool>,
    State(storage): State<Arc<Storage>>,
    session: Session,
    body: Body,
) -> Result<(StatusCode, Json<Upload>), ApiError> {
    ensure_writer(&session)?;
    let upload = axum::body::to_bytes(body, MAX_PDF_BYTES)
        .await
        .map_err(|_| ApiError::Status(StatusCode::PAYLOAD_TOO_LARGE))?;
    let id = storage.save_pdf(&pool, upload.into()).await?;
    Ok((StatusCode::CREATED, Json(Upload { id })))
}

/// How many Published documents a page of `/dokumen` holds.
const PAGE_SIZE: i64 = 10;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyDocumentFilter {
    /// Only documents of this group's types.
    #[param(inline)]
    group: Option<TransparencyDocumentGroup>,
    /// Only documents of this fiscal year.
    fiscal_year: Option<i32>,
    /// From 1, the default.
    page: Option<u32>,
}

/// A Published Transparency Document on `/dokumen`.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishedTransparencyDocument {
    title: String,
    document_type: TransparencyDocumentType,
    fiscal_year: i32,
    /// Served at `/media/{file}/pdf`.
    file: Uuid,
    byte_size: i64,
}

/// One page of `/dokumen`.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyDocumentPage {
    documents: Vec<PublishedTransparencyDocument>,
    /// How many Published documents match, over every page.
    total: i64,
    page_size: i64,
    /// Every fiscal year with a Published document, the latest first, for the year filter.
    fiscal_years: Vec<i32>,
}

/// The fiscal year's Published documents of the budget types, `APBDes`, its amendment and the
/// realisation report, the newest first, for its APBDes Summary.
pub(crate) async fn budget_documents(
    pool: &PgPool,
    fiscal_year: i32,
) -> sqlx::Result<Vec<PublishedTransparencyDocument>> {
    let types = [
        TransparencyDocumentType::Apbdes,
        TransparencyDocumentType::ApbdesAmendment,
        TransparencyDocumentType::RealisationReport,
    ];
    sqlx::query_as!(
        PublishedTransparencyDocument,
        r#"SELECT title, document_type AS "document_type: TransparencyDocumentType", fiscal_year,
           file_id AS file, byte_size AS "byte_size!"
           FROM transparency_documents documents JOIN files ON files.id = file_id
           WHERE published AND document_type = ANY($1) AND fiscal_year = $2
           ORDER BY documents.id DESC"#,
        &types as &[TransparencyDocumentType],
        fiscal_year,
    )
    .fetch_all(pool)
    .await
}

/// Published Transparency Documents, the latest fiscal year first, a page at a time.
#[utoipa::path(
    get,
    path = "/transparency-documents/published",
    operation_id = "listPublishedTransparencyDocuments",
    tag = "content",
    params(TransparencyDocumentFilter),
    responses(
        (status = OK, description = "A page of Published Transparency Documents", body = TransparencyDocumentPage),
        (status = BAD_REQUEST, description = "The filter is malformed"),
    ),
)]
pub(crate) async fn list_published(
    State(pool): State<PgPool>,
    Query(filter): Query<TransparencyDocumentFilter>,
) -> Result<Json<TransparencyDocumentPage>, ApiError> {
    let offset = (i64::from(filter.page.unwrap_or(1).max(1)) - 1) * PAGE_SIZE;
    let types = filter.group.map(TransparencyDocumentGroup::types);
    let total = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM transparency_documents
           WHERE published AND ($1::document_type[] IS NULL OR document_type = ANY($1))
           AND ($2::integer IS NULL OR fiscal_year = $2)"#,
        types as Option<&[TransparencyDocumentType]>,
        filter.fiscal_year,
    )
    .fetch_one(&pool)
    .await?;
    let documents = sqlx::query_as!(
        PublishedTransparencyDocument,
        r#"SELECT title, document_type AS "document_type: TransparencyDocumentType", fiscal_year,
           file_id AS file, byte_size AS "byte_size!"
           FROM transparency_documents documents JOIN files ON files.id = file_id
           WHERE published AND ($1::document_type[] IS NULL OR document_type = ANY($1))
           AND ($2::integer IS NULL OR fiscal_year = $2)
           ORDER BY fiscal_year DESC, documents.id DESC LIMIT $3 OFFSET $4"#,
        types as Option<&[TransparencyDocumentType]>,
        filter.fiscal_year,
        PAGE_SIZE,
        offset,
    )
    .fetch_all(&pool)
    .await?;
    let fiscal_years = sqlx::query_scalar!(
        "SELECT DISTINCT fiscal_year FROM transparency_documents WHERE published
         ORDER BY fiscal_year DESC"
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(TransparencyDocumentPage {
        documents,
        total,
        page_size: PAGE_SIZE,
        fiscal_years,
    }))
}
