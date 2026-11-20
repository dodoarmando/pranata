//! Pages: undated content under the public site's Profil, Pemerintahan and Layanan menus,
//! written as Drafts and then Published.
//!
//! `content.write` creates and edits Drafts; `content.publish` also publishes, unpublishes,
//! edits Published Pages and deletes Drafts.

use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    ApiError,
    audit::{self, Actor},
    auth::Session,
    category::ensure_publisher,
    government::Created,
    news_post::{self, ensure_writer, saved},
    rich_text::Doc,
    slug,
    storage::{self, Storage},
    village::{Upload, diff},
};

/// The fixed parent menu a Page sits under, which is also its URL's first part.
#[derive(Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "page_menu", rename_all = "lowercase")]
pub enum PageMenu {
    /// Profil, at `/profil`.
    Profile,
    /// Pemerintahan, at `/pemerintahan`.
    Government,
    /// Layanan, at `/layanan`.
    Services,
}

/// What a User writes in a Page. A Draft may leave out what publishing requires.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PageFields {
    /// 1–150 characters.
    title: String,
    /// Made from the title when blank; transliterated, and locked once first published.
    #[serde(default)]
    slug: Option<String>,
    /// Locked once first published, like the slug.
    menu: PageMenu,
    /// The place within the menu, smallest first; a new Page goes last when left out, and an
    /// edit keeps the place.
    #[serde(default)]
    order: Option<i32>,
    /// The meta description, at most 250 characters; required to publish.
    #[serde(default)]
    summary: Option<String>,
    /// The optional cover photo, uploaded at `/pages/cover`.
    #[serde(default)]
    cover: Option<Uuid>,
    /// Up to 300 characters.
    #[serde(default)]
    cover_caption: Option<String>,
    /// The cover's alt text, up to 300 characters; required to publish with a cover.
    #[serde(default)]
    cover_alt: Option<String>,
    /// Tiptap JSON of the allowed nodes and marks only: paragraph, h2–h3, bold, italic, link
    /// (`http(s):`, `mailto:`, `tel:`), bullet and ordered list, blockquote, table and `YouTube`.
    #[serde(default = "Doc::empty")]
    #[schema(value_type = Object)]
    body: Doc,
}

impl PageFields {
    /// Trims every text, blank to null, and refuses with `422` what is malformed.
    fn normalized(self) -> Result<Self, ApiError> {
        let text = |value: Option<String>, max: usize| match value.as_deref().map(str::trim) {
            None | Some("") => Ok(None),
            Some(value) if value.chars().count() <= max => Ok(Some(value.to_owned())),
            Some(_) => Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY)),
        };
        Ok(Self {
            title: text(Some(self.title), 150)?
                .ok_or(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))?,
            slug: self
                .slug
                .map(|slug| slug::of(&slug, ""))
                .filter(|slug| !slug.is_empty()),
            menu: self.menu,
            order: self.order,
            summary: text(self.summary, 250)?,
            cover: self.cover,
            cover_caption: text(self.cover_caption, 300)?,
            cover_alt: text(self.cover_alt, 300)?,
            body: self.body,
        })
    }

    /// The fields as an Audit Entry shows them, the body as plain text.
    fn audited(&self, slug: &str, order: i32) -> Value {
        json!({
            "title": self.title, "slug": slug, "menu": self.menu, "order": order,
            "summary": self.summary, "cover": self.cover, "coverCaption": self.cover_caption,
            "coverAlt": self.cover_alt, "body": self.body.plain_text(),
        })
    }
}

/// A Page as the admin area edits it.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    id: Uuid,
    #[serde(flatten)]
    fields: PageFields,
    published: bool,
    /// Null until first published; the slug and menu are locked once set.
    first_published_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

/// A line of the admin area's Page list.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PageSummary {
    id: Uuid,
    title: String,
    slug: String,
    menu: PageMenu,
    order: i32,
    published: bool,
    updated_at: DateTime<Utc>,
}

/// `base`, or `base-2`, `base-3`… when another Page than `except` has it.
async fn free_slug(
    conn: &mut sqlx::PgConnection,
    base: &str,
    except: Option<Uuid>,
) -> sqlx::Result<String> {
    // `base` is a slug, so it holds nothing a regular expression would read.
    let taken = sqlx::query_scalar!(
        "SELECT slug FROM pages WHERE (slug = $1 OR slug ~ ('^' || $1 || '-[0-9]+$'))
         AND id IS DISTINCT FROM $2",
        base,
        except,
    )
    .fetch_all(conn)
    .await?;
    Ok(slug::first_free(base, &taken))
}

/// Every Page, Drafts and Published, menu by menu in order.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`.
#[utoipa::path(
    get,
    path = "/pages",
    operation_id = "listPages",
    tag = "content",
    responses(
        (status = OK, description = "Every Page", body = Vec<PageSummary>),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
    ),
)]
pub(crate) async fn list(
    State(pool): State<PgPool>,
    session: Session,
) -> Result<Json<Vec<PageSummary>>, ApiError> {
    ensure_writer(&session)?;
    let pages = sqlx::query_as!(
        PageSummary,
        r#"SELECT id, title, slug, menu AS "menu: PageMenu", sort_order AS "order", published,
           updated_at FROM pages ORDER BY menu, sort_order, title, id"#
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(pages))
}

/// One Page, Draft or Published, for editing.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `404`
/// for an unknown Page.
#[utoipa::path(
    get,
    path = "/pages/{id}",
    operation_id = "getPage",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Page")),
    responses(
        (status = OK, description = "The Page", body = Page),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
        (status = NOT_FOUND, description = "There is no such Page"),
    ),
)]
pub(crate) async fn get(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<Json<Page>, ApiError> {
    ensure_writer(&session)?;
    let row = sqlx::query!(
        r#"SELECT title, slug, menu AS "menu: PageMenu", sort_order, summary, cover_file_id,
           cover_caption, cover_alt, body, published, first_published_at, updated_at
           FROM pages WHERE id = $1"#,
        id
    )
    .fetch_optional(&pool)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    Ok(Json(Page {
        id,
        fields: PageFields {
            title: row.title,
            slug: Some(row.slug),
            menu: row.menu,
            order: Some(row.sort_order),
            summary: row.summary,
            cover: row.cover_file_id,
            cover_caption: row.cover_caption,
            cover_alt: row.cover_alt,
            body: serde_json::from_value(row.body)?,
        },
        published: row.published,
        first_published_at: row.first_published_at,
        updated_at: row.updated_at,
    }))
}

/// Starts a Draft written by the signed-in User; only the title and menu are required.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `409`
/// when another Page takes the slug meanwhile, `422` for a blank title, a missing or unknown
/// menu, a malformed field, a body outside the allowlist, or an unknown cover.
#[utoipa::path(
    post,
    path = "/pages",
    operation_id = "createPage",
    tag = "content",
    request_body = PageFields,
    responses(
        (status = CREATED, description = "The new Draft", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
        (status = CONFLICT, description = "Another Page took the slug meanwhile"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed or unknown"),
    ),
)]
pub(crate) async fn create(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<PageFields>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure_writer(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let base = new
        .slug
        .clone()
        .unwrap_or_else(|| slug::of(&new.title, "halaman"));
    let slug = free_slug(&mut tx, &base, None).await?;
    let created = saved(
        sqlx::query!(
            "INSERT INTO pages (title, slug, menu, sort_order, summary, cover_file_id,
             cover_caption, cover_alt, body, author_id)
             VALUES ($1, $2, $3, coalesce($4,
                 (SELECT coalesce(max(sort_order), 0) + 1 FROM pages WHERE menu = $3)),
             $5, $6, $7, $8, $9, $10) RETURNING id, sort_order",
            new.title,
            slug,
            new.menu as PageMenu,
            new.order,
            new.summary,
            new.cover,
            new.cover_caption,
            new.cover_alt,
            serde_json::to_value(&new.body)?,
            session.user_id,
        )
        .fetch_one(&mut *tx)
        .await,
    )?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "page.create",
        ("page", Some(&created.id.to_string())),
        diff(&Value::Null, &new.audited(&slug, created.sort_order)).into(),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id: created.id })))
}

/// Saves a Page: a Draft as it autosaves, a Published one live.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` (or `content.publish`) for a
/// Draft or without `content.publish` for a Published Page, `404` for an unknown Page, `409`
/// for a new slug or menu once first published or a slug another Page takes meanwhile, `422`
/// for a blank title, a malformed field, a body outside the allowlist, an unknown cover, or a
/// Published Page missing what publishing requires.
#[utoipa::path(
    put,
    path = "/pages/{id}",
    operation_id = "updatePage",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Page")),
    request_body = PageFields,
    responses(
        (status = NO_CONTENT, description = "The Page is saved"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not change this Page"),
        (status = NOT_FOUND, description = "There is no such Page"),
        (status = CONFLICT, description = "The slug or menu is locked, or the slug is taken"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed, unknown or missing"),
    ),
)]
pub(crate) async fn update(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<PageFields>,
) -> Result<StatusCode, ApiError> {
    ensure_writer(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        r#"SELECT title, slug, menu AS "menu: PageMenu", sort_order, summary, cover_file_id,
           cover_caption, cover_alt, body, published, first_published_at
           FROM pages WHERE id = $1 FOR UPDATE"#,
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if old.published {
        ensure_publisher(&session)?;
    }
    let slug = if old.first_published_at.is_some() {
        if new.menu != old.menu || new.slug.as_ref().is_some_and(|slug| *slug != old.slug) {
            return Err(ApiError::Status(StatusCode::CONFLICT));
        }
        old.slug.clone()
    } else {
        let base = new
            .slug
            .clone()
            .unwrap_or_else(|| slug::of(&new.title, "halaman"));
        free_slug(&mut tx, &base, Some(id)).await?
    };
    let order = new.order.unwrap_or(old.sort_order);
    let result = sqlx::query!(
        "UPDATE pages SET title = $1, slug = $2, menu = $3, sort_order = $4, summary = $5,
         cover_file_id = $6, cover_caption = $7, cover_alt = $8, body = $9, updated_at = now()
         WHERE id = $10",
        new.title,
        slug,
        new.menu as PageMenu,
        order,
        new.summary,
        new.cover,
        new.cover_caption,
        new.cover_alt,
        serde_json::to_value(&new.body)?,
        id,
    )
    .execute(&mut *tx)
    .await;
    match result {
        // A Published Page lost what publishing requires.
        Err(sqlx::Error::Database(error)) if error.is_check_violation() => {
            return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
        }
        result => saved(result)?,
    };
    if let Some(cover) = old.cover_file_id.filter(|&cover| Some(cover) != new.cover) {
        storage::release(&mut tx, cover).await?;
    }
    let from = PageFields {
        title: old.title,
        slug: None,
        menu: old.menu,
        order: None,
        summary: old.summary,
        cover: old.cover_file_id,
        cover_caption: old.cover_caption,
        cover_alt: old.cover_alt,
        body: serde_json::from_value(old.body)?,
    }
    .audited(&old.slug, old.sort_order);
    let changes = diff(&from, &new.audited(&slug, order));
    if !changes.is_empty() {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "page.update",
            ("page", Some(&id.to_string())),
            changes.into(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Puts a Draft on the public site; the first publication locks its slug and menu.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown Page,
/// `409` when it is already Published, `422` when it lacks a summary, or has a cover without
/// alt text.
#[utoipa::path(
    post,
    path = "/pages/{id}/publish",
    operation_id = "publishPage",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Page")),
    responses(
        (status = NO_CONTENT, description = "The Page is Published"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Page"),
        (status = CONFLICT, description = "The Page is already Published"),
        (status = UNPROCESSABLE_ENTITY, description = "A field publishing requires is missing"),
    ),
)]
pub(crate) async fn publish(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    set_published(&pool, &session, id, true).await
}

/// Takes a Published Page off the public site, back to Draft; it keeps its slug and menu.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown Page,
/// `409` when it is a Draft.
#[utoipa::path(
    post,
    path = "/pages/{id}/unpublish",
    operation_id = "unpublishPage",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Page")),
    responses(
        (status = NO_CONTENT, description = "The Page is a Draft again"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Page"),
        (status = CONFLICT, description = "The Page is a Draft"),
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
    let was = sqlx::query_scalar!("SELECT published FROM pages WHERE id = $1 FOR UPDATE", id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if was == published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    let result = sqlx::query!(
        "UPDATE pages SET published = $1,
         first_published_at = coalesce(first_published_at, now()) WHERE id = $2",
        published,
        id,
    )
    .execute(&mut *tx)
    .await;
    match result {
        Err(sqlx::Error::Database(error)) if error.is_check_violation() => {
            return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
        }
        result => result?,
    };
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        if published {
            "page.publish"
        } else {
            "page.unpublish"
        },
        ("page", Some(&id.to_string())),
        json!([{ "field": "published", "from": was, "to": published }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a Draft for good; its slug is free again. A Published Page is unpublished first.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown Page,
/// `409` when it is Published.
#[utoipa::path(
    delete,
    path = "/pages/{id}",
    operation_id = "deletePage",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Page")),
    responses(
        (status = NO_CONTENT, description = "The Draft is deleted"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Page"),
        (status = CONFLICT, description = "The Page is Published"),
    ),
)]
pub(crate) async fn delete(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    ensure_publisher(&session)?;
    let mut tx = pool.begin().await?;
    let published = sqlx::query_scalar!("SELECT published FROM pages WHERE id = $1 FOR UPDATE", id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    let old = sqlx::query!(
        "DELETE FROM pages WHERE id = $1 RETURNING title, cover_file_id",
        id
    )
    .fetch_one(&mut *tx)
    .await?;
    if let Some(cover) = old.cover_file_id {
        storage::release(&mut tx, cover).await?;
    }
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "page.delete",
        ("page", Some(&id.to_string())),
        json!([{ "field": "title", "from": old.title, "to": null }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Stores a cover photo, a JPEG, PNG or WebP of at most 10 MiB, for saving into a Page.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `413`
/// over 10 MiB, `415` for content that is not JPEG, PNG or WebP, `422` over 40 megapixels or
/// undecodable.
#[utoipa::path(
    post,
    path = "/pages/cover",
    operation_id = "uploadPageCover",
    tag = "content",
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = CREATED, description = "The stored photo", body = Upload),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
        (status = PAYLOAD_TOO_LARGE, description = "The file is over 10 MiB"),
        (status = UNSUPPORTED_MEDIA_TYPE, description = "The file is not JPEG, PNG or WebP"),
        (status = UNPROCESSABLE_ENTITY, description = "The image is too large or broken"),
    ),
)]
pub(crate) async fn upload_cover(
    pool: State<PgPool>,
    storage: State<Arc<Storage>>,
    session: Session,
    body: Body,
) -> Result<(StatusCode, Json<Upload>), ApiError> {
    // The same photo rules as a News Post's cover.
    news_post::upload_cover(pool, storage, session, body).await
}

/// A Published Page in the public navigation.
#[derive(Serialize, ToSchema)]
pub struct PageLink {
    menu: PageMenu,
    slug: String,
    title: String,
}

/// Published Pages for the public navigation: Profil, then Pemerintahan, then Layanan, each in
/// order.
#[utoipa::path(
    get,
    path = "/pages/published",
    operation_id = "listPublishedPages",
    tag = "content",
    responses((status = OK, description = "Every Published Page", body = Vec<PageLink>)),
)]
pub(crate) async fn list_published(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<PageLink>>, ApiError> {
    let pages = sqlx::query_as!(
        PageLink,
        r#"SELECT menu AS "menu: PageMenu", slug, title FROM pages WHERE published
           ORDER BY menu, sort_order, title, id"#
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(pages))
}

/// A Published Page as the public reads it.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishedPage {
    slug: String,
    title: String,
    menu: PageMenu,
    summary: String,
    cover: Option<Uuid>,
    cover_caption: Option<String>,
    cover_alt: Option<String>,
    /// Tiptap JSON of the allowed nodes and marks only.
    #[schema(value_type = Object)]
    body: Value,
    /// The last save, shown as "last updated".
    updated_at: DateTime<Utc>,
}

/// One Published Page by its slug.
///
/// # Errors
/// `404` for a slug no Published Page has, such as a Draft's or a deleted one's.
#[utoipa::path(
    get,
    path = "/pages/published/{slug}",
    operation_id = "getPublishedPage",
    tag = "content",
    params(("slug" = String, Path, description = "The Page's slug")),
    responses(
        (status = OK, description = "The Page", body = PublishedPage),
        (status = NOT_FOUND, description = "No Published Page has the slug"),
    ),
)]
pub(crate) async fn get_published(
    State(pool): State<PgPool>,
    Path(slug): Path<String>,
) -> Result<Json<PublishedPage>, ApiError> {
    let page = sqlx::query_as!(
        PublishedPage,
        r#"SELECT slug, title, menu AS "menu: PageMenu", summary AS "summary!",
           cover_file_id AS cover, cover_caption, cover_alt, body, updated_at
           FROM pages WHERE published AND slug = $1"#,
        slug,
    )
    .fetch_optional(&pool)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    Ok(Json(page))
}
