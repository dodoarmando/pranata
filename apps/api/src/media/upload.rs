//! `POST /api/admin/media` and `GET /api/admin/media/{id}`.

use axum::{
    Json,
    extract::{DefaultBodyLimit, Multipart, Path, State, multipart::MultipartError},
    http::StatusCode,
};
use bytes::Bytes;
use serde_json::Value;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use super::{
    Format, MediaAsset, hex, load, original_path,
    pipeline::{self},
};
use crate::{
    ApiError, AppState, FieldError,
    audit::{self, AuditAction, FieldChange, NewAuditEntry},
    auth::CurrentUser,
    error::Problem,
    jobs::{self, Job},
};

/// Images stop at 10 MB (D-16).
pub const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
/// Documents (PDF) stop at 20 MB (D-16).
pub const MAX_DOCUMENT_BYTES: usize = 20 * 1024 * 1024;
/// Room for the multipart framing around the largest file.
const MULTIPART_OVERHEAD: usize = 1024 * 1024;

/// The longest original file name kept.
const MAX_NAME_CHARS: usize = 200;

/// The multipart form of an upload.
#[derive(ToSchema)]
#[allow(dead_code)] // documents the form; the handler reads the parts itself
pub struct UploadForm {
    /// The file. JPEG, PNG or WebP up to 10 MB for `image`; PDF up to 20 MB
    /// for `document`. The type is recognised from the content.
    #[schema(format = Binary, value_type = String)]
    file: Vec<u8>,
    /// `image` (default) or `document`.
    kind: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Image,
    Document,
}

impl Kind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Document => "document",
        }
    }

    const fn max_bytes(self) -> usize {
        match self {
            Self::Image => MAX_IMAGE_BYTES,
            Self::Document => MAX_DOCUMENT_BYTES,
        }
    }
}

struct Form {
    file: Bytes,
    name: String,
    kind: Kind,
}

fn invalid(field: &str, code: &str) -> ApiError {
    ApiError::Validation(vec![FieldError {
        field: field.to_owned(),
        code: code.to_owned(),
    }])
}

fn multipart_error(error: &MultipartError) -> ApiError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::TooLarge("file_too_large")
    } else {
        invalid("file", "invalid_multipart")
    }
}

/// Keep the base name only, without control characters, and bound its length.
fn clean_name(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "upload".to_owned()
    } else {
        cleaned.to_owned()
    }
}

async fn read_form(mut multipart: Multipart) -> Result<Form, ApiError> {
    let mut file: Option<(Bytes, String)> = None;
    let mut kind = Kind::Image;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| multipart_error(&e))?
    {
        match field.name() {
            Some("file") => {
                if file.is_some() {
                    return Err(invalid("file", "duplicate"));
                }
                let name = clean_name(field.file_name().unwrap_or_default());
                let mut data = Vec::new();
                while let Some(chunk) = field.chunk().await.map_err(|e| multipart_error(&e))? {
                    if data.len() + chunk.len() > MAX_DOCUMENT_BYTES {
                        return Err(ApiError::TooLarge("file_too_large"));
                    }
                    data.extend_from_slice(&chunk);
                }
                file = Some((Bytes::from(data), name));
            }
            Some("kind") => {
                let text = field.text().await.map_err(|e| multipart_error(&e))?;
                kind = match text.trim() {
                    "image" | "" => Kind::Image,
                    "document" => Kind::Document,
                    _ => return Err(invalid("kind", "invalid")),
                };
            }
            _ => {}
        }
    }
    let (file, name) = file.ok_or_else(|| invalid("file", "required"))?;
    if file.is_empty() {
        return Err(invalid("file", "required"));
    }
    Ok(Form { file, name, kind })
}

/// Upload an image or a document.
///
/// An image is accepted as `pending` and turns `ready` once its background job
/// has produced the metadata-free original and the WebP variants. The same
/// file (by SHA-256) answers 200 with the existing asset.
///
/// # Errors
/// 413 `file_too_large`, 415 `unsupported_media_type`, 422 `image_too_large` or
/// `image_unreadable`, 422 `validation_failed` for a bad form.
#[utoipa::path(
    post,
    path = "/admin/media",
    operation_id = "uploadMedia",
    tag = "media",
    request_body(content = UploadForm, content_type = "multipart/form-data"),
    params(("x-csrf-token" = String, Header, description = "The CSRF token from sign-in")),
    security(("session" = [])),
    responses(
        (status = 201, description = "A new asset", body = MediaAsset),
        (status = 200, description = "The same file was uploaded before; the existing asset", body = MediaAsset),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Missing or wrong CSRF token (code csrf_mismatch)", body = Problem, content_type = "application/problem+json"),
        (status = 413, description = "Over 10 MB for an image or 20 MB for a document (code file_too_large)", body = Problem, content_type = "application/problem+json"),
        (status = 415, description = "Not JPEG, PNG or WebP for an image, or not PDF for a document (code unsupported_media_type)", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Over 50 megapixels (code image_too_large), an unreadable image (code image_unreadable) or a bad form (code validation_failed)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn upload_media(
    State(state): State<AppState>,
    user: CurrentUser,
    multipart: Multipart,
) -> Result<(StatusCode, Json<MediaAsset>), ApiError> {
    let Form { file, name, kind } = read_form(multipart).await?;

    if file.len() > kind.max_bytes() {
        return Err(ApiError::TooLarge("file_too_large"));
    }
    // The kind and the content must agree; the content decides what a file is.
    let sniffed = Format::sniff(&file).ok_or(ApiError::Unsupported("unsupported_media_type"))?;
    if sniffed.is_image() != (kind == Kind::Image) {
        return Err(ApiError::Unsupported("unsupported_media_type"));
    }
    if kind == Kind::Image {
        // Header only: dimensions and the pixel limit, before any decoding.
        pipeline::inspect(&file)?;
    }

    let sha256 = Sha256::digest(&file).to_vec();
    let hex = hex(&sha256);
    let status = match kind {
        Kind::Image => "pending",
        Kind::Document => "ready",
    };
    let size = i64::try_from(file.len()).map_err(|e| ApiError::Internal(e.into()))?;

    // The row, the original, the audit entry and the job commit together. The
    // unique SHA-256 makes a concurrent duplicate wait here and then see the
    // winner's row, so the original is only ever written by the winner.
    let mut tx = state.db.begin().await?;
    let inserted = sqlx::query_scalar!(
        "INSERT INTO media_assets (sha256, kind, mime, original_name, bytes, status, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (sha256) DO NOTHING
         RETURNING id",
        sha256,
        kind.as_str(),
        sniffed.mime(),
        name,
        size,
        status,
        user.id.0,
    )
    .fetch_optional(&mut *tx)
    .await?;

    let Some(id) = inserted else {
        // The same file again. A failed image gets another try with the new
        // bytes; anything else answers with the existing asset.
        let existing = sqlx::query!(
            "SELECT id, status FROM media_assets WHERE sha256 = $1 FOR UPDATE",
            sha256
        )
        .fetch_one(&mut *tx)
        .await?;
        if existing.status == "failed" {
            retry_failed(
                &state,
                &mut tx,
                &user,
                existing.id,
                &original_path(&hex, sniffed),
                file,
                &name,
            )
            .await?;
        }
        tx.commit().await?;
        let asset = load(&state.db, existing.id)
            .await?
            .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("asset vanished")))?;
        return Ok((StatusCode::OK, Json(asset)));
    };

    state
        .storage
        .put(&original_path(&hex, sniffed), file)
        .await?;
    audit::record(
        &mut tx,
        NewAuditEntry {
            actor: Some(user.id),
            action: AuditAction::Create,
            object_type: "media_asset",
            object_id: id.to_string(),
            summary: format!("Uploaded {} {name}", kind.as_str()),
            changes: vec![
                created("kind", Value::from(kind.as_str())),
                created("original_name", Value::from(name.clone())),
                created("bytes", Value::from(size)),
            ],
        },
    )
    .await?;
    if kind == Kind::Image {
        jobs::enqueue(
            &mut tx,
            Job::ProcessImage { asset_id: id },
            OffsetDateTime::now_utc(),
        )
        .await?;
    }
    tx.commit().await?;

    let asset = load(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("asset vanished")))?;
    Ok((StatusCode::CREATED, Json(asset)))
}

/// Give a failed image another go with the newly uploaded bytes: back to
/// `pending`, original rewritten, audit entry and fresh job, in the caller's
/// transaction.
async fn retry_failed(
    state: &AppState,
    tx: &mut sqlx::PgConnection,
    user: &CurrentUser,
    id: Uuid,
    original: &str,
    file: Bytes,
    name: &str,
) -> Result<(), ApiError> {
    state.storage.put(original, file).await?;
    sqlx::query!(
        "UPDATE media_assets SET status = 'pending' WHERE id = $1",
        id
    )
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut *tx,
        NewAuditEntry {
            actor: Some(user.id),
            action: AuditAction::Update,
            object_type: "media_asset",
            object_id: id.to_string(),
            summary: format!("Retried processing of {name}"),
            changes: vec![FieldChange {
                field: "status".to_owned(),
                before: Value::from("failed"),
                after: Value::from("pending"),
            }],
        },
    )
    .await?;
    jobs::enqueue(
        &mut *tx,
        Job::ProcessImage { asset_id: id },
        OffsetDateTime::now_utc(),
    )
    .await?;
    Ok(())
}

fn created(field: &str, after: Value) -> FieldChange {
    FieldChange {
        field: field.to_owned(),
        before: Value::Null,
        after,
    }
}

/// Read one asset.
///
/// # Errors
/// 404 `media_not_found`.
#[utoipa::path(
    get,
    path = "/admin/media/{id}",
    operation_id = "getMedia",
    tag = "media",
    params(("id" = Uuid, Path, description = "Asset id")),
    security(("session" = [])),
    responses(
        (status = 200, description = "The asset", body = MediaAsset),
        (status = 401, description = "Not signed in (code unauthenticated)", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such asset (code media_not_found)", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn get_media(
    State(state): State<AppState>,
    _user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<MediaAsset>, ApiError> {
    load(&state.db, id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound("media_not_found"))
}

/// Admin media routes. Only the upload route accepts a large body; every
/// other route keeps the default limit.
#[must_use]
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(upload_media))
        .layer(DefaultBodyLimit::max(
            MAX_DOCUMENT_BYTES + MULTIPART_OVERHEAD,
        ))
        .merge(OpenApiRouter::new().routes(routes!(get_media)))
}
