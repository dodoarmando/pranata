//! Partners: organisations whose logos appear in the home page partner strip, saved as Drafts
//! and then Published, in an order publishers drag them into.
//!
//! `content.write` creates and edits Drafts and uploads logos; `content.publish` also
//! publishes, unpublishes, edits Published Partners, deletes Drafts and reorders them.

use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{
    ApiError, AppState,
    audit::{self, Actor},
    auth::Session,
    category::ensure_publisher,
    government::Created,
    imaging::Kind,
    news_post::{ensure_writer, saved},
    order::Order,
    storage::{self, Storage},
    village::{Upload, diff},
};

pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(update, delete))
        .routes(routes!(publish))
        .routes(routes!(unpublish))
        .routes(routes!(order))
        .routes(routes!(upload_logo))
        .routes(routes!(list_published))
}

/// What a User writes in a Partner.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PartnerFields {
    /// 1–100 characters; the logo's alt text.
    name: String,
    /// The logo, uploaded at `/partners/logo`.
    logo: Uuid,
    /// The Partner's website, `http(s)://` and at most 300 characters.
    #[serde(default)]
    url: Option<String>,
}

impl PartnerFields {
    /// Trims every text, blank to null, and refuses with `422` what is malformed.
    fn normalized(self) -> Result<Self, ApiError> {
        let name = self.name.trim();
        let url = self
            .url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty());
        let web = |url: &str| {
            let lower = url.to_ascii_lowercase();
            let rest = lower
                .strip_prefix("https://")
                .or_else(|| lower.strip_prefix("http://"));
            url.len() <= 300
                && rest.is_some_and(|rest| !rest.is_empty())
                && !url.chars().any(char::is_whitespace)
        };
        if name.is_empty() || name.chars().count() > 100 || url.is_some_and(|url| !web(url)) {
            return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
        }
        Ok(Self {
            name: name.to_owned(),
            logo: self.logo,
            url: url.map(str::to_owned),
        })
    }

    fn audited(&self) -> Value {
        json!({ "name": self.name, "logo": self.logo, "url": self.url })
    }
}

/// A Partner as the admin area lists and edits it.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Partner {
    id: Uuid,
    #[serde(flatten)]
    fields: PartnerFields,
    published: bool,
}

/// Every Partner, Drafts and Published, in order.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`.
#[utoipa::path(
    get,
    path = "/partners",
    operation_id = "listPartners",
    tag = "content",
    responses(
        (status = OK, description = "Every Partner", body = Vec<Partner>),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
    ),
)]
pub(crate) async fn list(
    State(pool): State<PgPool>,
    session: Session,
) -> Result<Json<Vec<Partner>>, ApiError> {
    ensure_writer(&session)?;
    let rows = sqlx::query!(
        "SELECT id, name, logo_file_id, url, published FROM partners ORDER BY sort_order, id"
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| Partner {
                id: row.id,
                fields: PartnerFields {
                    name: row.name,
                    logo: row.logo_file_id,
                    url: row.url,
                },
                published: row.published,
            })
            .collect(),
    ))
}

/// Saves a new Partner as a Draft, last.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `422`
/// for a blank name, an unknown logo or a URL that is not `http(s)://`.
#[utoipa::path(
    post,
    path = "/partners",
    operation_id = "createPartner",
    tag = "content",
    request_body = PartnerFields,
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
    Json(body): Json<PartnerFields>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure_writer(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let id = saved(
        sqlx::query_scalar!(
            "INSERT INTO partners (name, logo_file_id, url, sort_order)
             VALUES ($1, $2, $3, (SELECT coalesce(max(sort_order), 0) + 1 FROM partners))
             RETURNING id",
            new.name,
            new.logo,
            new.url,
        )
        .fetch_one(&mut *tx)
        .await,
    )?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "partner.create",
        ("partner", Some(&id.to_string())),
        diff(&Value::Null, &new.audited()).into(),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

/// Saves a Partner, a Draft or a Published one live.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` (or `content.publish`) for a
/// Draft or without `content.publish` for a Published Partner, `404` for an unknown Partner,
/// `422` for a blank name, an unknown logo or a URL that is not `http(s)://`.
#[utoipa::path(
    put,
    path = "/partners/{id}",
    operation_id = "updatePartner",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Partner")),
    request_body = PartnerFields,
    responses(
        (status = NO_CONTENT, description = "The Partner is saved"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not change this Partner"),
        (status = NOT_FOUND, description = "There is no such Partner"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed or unknown"),
    ),
)]
pub(crate) async fn update(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<PartnerFields>,
) -> Result<StatusCode, ApiError> {
    ensure_writer(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        "SELECT name, logo_file_id, url, published FROM partners WHERE id = $1 FOR UPDATE",
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
            "UPDATE partners SET name = $1, logo_file_id = $2, url = $3, updated_at = now()
             WHERE id = $4",
            new.name,
            new.logo,
            new.url,
            id,
        )
        .execute(&mut *tx)
        .await,
    )?;
    if old.logo_file_id != new.logo {
        storage::release(&mut tx, old.logo_file_id).await?;
    }
    let from = PartnerFields {
        name: old.name,
        logo: old.logo_file_id,
        url: old.url,
    }
    .audited();
    let changes = diff(&from, &new.audited());
    if !changes.is_empty() {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "partner.update",
            ("partner", Some(&id.to_string())),
            changes.into(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Puts a Draft in the home page partner strip.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// Partner, `409` when it is already Published.
#[utoipa::path(
    post,
    path = "/partners/{id}/publish",
    operation_id = "publishPartner",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Partner")),
    responses(
        (status = NO_CONTENT, description = "The Partner is Published"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Partner"),
        (status = CONFLICT, description = "The Partner is already Published"),
    ),
)]
pub(crate) async fn publish(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    set_published(&pool, &session, id, true).await
}

/// Takes a Published Partner out of the partner strip, back to Draft.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// Partner, `409` when it is a Draft.
#[utoipa::path(
    post,
    path = "/partners/{id}/unpublish",
    operation_id = "unpublishPartner",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Partner")),
    responses(
        (status = NO_CONTENT, description = "The Partner is a Draft again"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Partner"),
        (status = CONFLICT, description = "The Partner is a Draft"),
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
        "SELECT published FROM partners WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if was == published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    sqlx::query!(
        "UPDATE partners SET published = $1 WHERE id = $2",
        published,
        id
    )
    .execute(&mut *tx)
    .await?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        if published {
            "partner.publish"
        } else {
            "partner.unpublish"
        },
        ("partner", Some(&id.to_string())),
        json!([{ "field": "published", "from": was, "to": published }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a Draft for good. A Published Partner is unpublished first.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// Partner, `409` when it is Published.
#[utoipa::path(
    delete,
    path = "/partners/{id}",
    operation_id = "deletePartner",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Partner")),
    responses(
        (status = NO_CONTENT, description = "The Draft is deleted"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Partner"),
        (status = CONFLICT, description = "The Partner is Published"),
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
        "SELECT name, logo_file_id, published FROM partners WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if old.published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    sqlx::query!("DELETE FROM partners WHERE id = $1", id)
        .execute(&mut *tx)
        .await?;
    storage::release(&mut tx, old.logo_file_id).await?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "partner.delete",
        ("partner", Some(&id.to_string())),
        json!([{ "field": "name", "from": old.name, "to": null }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Puts every Partner, Draft and Published, in a new order.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `422` unless the list holds
/// every Partner exactly once.
#[utoipa::path(
    put,
    path = "/partners/order",
    operation_id = "orderPartners",
    tag = "content",
    request_body = Order,
    responses(
        (status = NO_CONTENT, description = "The Partners are reordered"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = UNPROCESSABLE_ENTITY, description = "The list is not every Partner exactly once"),
    ),
)]
pub(crate) async fn order(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<Order>,
) -> Result<StatusCode, ApiError> {
    ensure_publisher(&session)?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query_scalar!("SELECT id FROM partners ORDER BY sort_order, id FOR UPDATE")
        .fetch_all(&mut *tx)
        .await?;
    if !body.matches(&old) {
        return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
    }
    sqlx::query!(
        "UPDATE partners SET sort_order = ordered.sort_order
         FROM unnest($1::uuid[]) WITH ORDINALITY AS ordered (id, sort_order)
         WHERE partners.id = ordered.id",
        &body.ids,
    )
    .execute(&mut *tx)
    .await?;
    if old != body.ids {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "partner.order",
            ("partner", None),
            json!([{ "field": "order", "from": old, "to": body.ids }]),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Stores a logo, a PNG (or an SVG, rasterised) of at most 2 MiB, for saving into a Partner.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `413`
/// over 2 MiB, `415` for content that is not PNG or SVG, `422` over 40 megapixels or
/// undecodable.
#[utoipa::path(
    post,
    path = "/partners/logo",
    operation_id = "uploadPartnerLogo",
    tag = "content",
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = CREATED, description = "The stored logo", body = Upload),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
        (status = PAYLOAD_TOO_LARGE, description = "The file is over 2 MiB"),
        (status = UNSUPPORTED_MEDIA_TYPE, description = "The file is not PNG or SVG"),
        (status = UNPROCESSABLE_ENTITY, description = "The image is too large or broken"),
    ),
)]
pub(crate) async fn upload_logo(
    State(pool): State<PgPool>,
    State(storage): State<Arc<Storage>>,
    session: Session,
    body: Body,
) -> Result<(StatusCode, Json<Upload>), ApiError> {
    ensure_writer(&session)?;
    let upload = axum::body::to_bytes(body, Kind::Logo.max_bytes())
        .await
        .map_err(|_| ApiError::Status(StatusCode::PAYLOAD_TOO_LARGE))?;
    let id = storage.save_image(&pool, Kind::Logo, upload.into()).await?;
    Ok((StatusCode::CREATED, Json(Upload { id })))
}

/// A Published Partner in the home page partner strip.
#[derive(Serialize, ToSchema)]
pub struct PublishedPartner {
    name: String,
    /// Served at `/media/{logo}/{variant}`.
    logo: Uuid,
    url: Option<String>,
}

/// Published Partners in order, for the home page partner strip.
#[utoipa::path(
    get,
    path = "/partners/published",
    operation_id = "listPublishedPartners",
    tag = "content",
    responses((status = OK, description = "Every Published Partner", body = Vec<PublishedPartner>)),
)]
pub(crate) async fn list_published(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<PublishedPartner>>, ApiError> {
    let partners = sqlx::query_as!(
        PublishedPartner,
        "SELECT name, logo_file_id AS logo, url FROM partners WHERE published
         ORDER BY sort_order, id"
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(partners))
}
