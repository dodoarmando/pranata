//! News Posts: dated reports on the public site, written as Drafts and then Published.
//!
//! `content.write` creates and edits Drafts; `content.publish` also publishes, unpublishes,
//! edits Published posts and deletes Drafts.

use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    ApiError,
    audit::{self, Actor},
    auth::Session,
    category::ensure_publisher,
    government::Created,
    imaging::Kind,
    rich_text::Doc,
    role::Permission,
    slug,
    storage::{self, Storage},
    village::{Upload, diff},
};

/// What a User writes in a News Post. A Draft may leave out what publishing requires.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NewsPostFields {
    /// 1–150 characters.
    title: String,
    /// Made from the title when blank; transliterated, and locked once first published.
    #[serde(default)]
    slug: Option<String>,
    /// Required to publish.
    #[serde(default)]
    category: Option<Uuid>,
    /// The lede and meta description, about 200 characters and at most 250; required to
    /// publish.
    #[serde(default)]
    summary: Option<String>,
    /// The cover photo, uploaded at `/news-posts/cover`; required to publish.
    #[serde(default)]
    cover: Option<Uuid>,
    /// Up to 300 characters.
    #[serde(default)]
    cover_caption: Option<String>,
    /// The cover's alt text, up to 300 characters; required to publish.
    #[serde(default)]
    cover_alt: Option<String>,
    /// Tiptap JSON of the allowed nodes and marks only: paragraph, h2–h3, bold, italic, link
    /// (`http(s):`, `mailto:`, `tel:`), bullet and ordered list, blockquote, table and `YouTube`.
    #[serde(default = "Doc::empty")]
    #[schema(value_type = Object)]
    body: Doc,
}

impl NewsPostFields {
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
            category: self.category,
            summary: text(self.summary, 250)?,
            cover: self.cover,
            cover_caption: text(self.cover_caption, 300)?,
            cover_alt: text(self.cover_alt, 300)?,
            body: self.body,
        })
    }

    /// The fields as an Audit Entry shows them, the body as plain text.
    fn audited(&self, slug: &str) -> Value {
        json!({
            "title": self.title, "slug": slug, "category": self.category,
            "summary": self.summary, "cover": self.cover, "coverCaption": self.cover_caption,
            "coverAlt": self.cover_alt, "body": self.body.plain_text(),
        })
    }
}

/// A News Post as the admin area edits it.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewsPost {
    id: Uuid,
    #[serde(flatten)]
    fields: NewsPostFields,
    published: bool,
    /// The public date, kept through an unpublish; null until first published, and the slug is
    /// locked once set.
    first_published_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

/// A line of the admin area's News Post list.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewsPostSummary {
    id: Uuid,
    title: String,
    slug: String,
    /// The Category's name.
    category: Option<String>,
    published: bool,
    first_published_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
    /// The creating User's username.
    author: String,
}

/// Editing a Draft is writing; `content.publish` covers it too.
pub(crate) fn ensure_writer(session: &Session) -> Result<(), ApiError> {
    if session.holds(Permission::ContentWrite) || session.holds(Permission::ContentPublish) {
        Ok(())
    } else {
        Err(ApiError::Status(StatusCode::FORBIDDEN))
    }
}

/// An unknown Category or cover says the body is wrong; a slug taken meanwhile, a conflict.
pub(crate) fn saved<T>(result: sqlx::Result<T>) -> Result<T, ApiError> {
    match result {
        Err(sqlx::Error::Database(error)) if error.is_foreign_key_violation() => {
            Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))
        }
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(ApiError::Status(StatusCode::CONFLICT))
        }
        result => Ok(result?),
    }
}

/// `base`, or `base-2`, `base-3`… when another News Post than `except` has it.
async fn free_slug(
    conn: &mut sqlx::PgConnection,
    base: &str,
    except: Option<Uuid>,
) -> sqlx::Result<String> {
    // `base` is a slug, so it holds nothing a regular expression would read.
    let taken = sqlx::query_scalar!(
        "SELECT slug FROM news_posts WHERE (slug = $1 OR slug ~ ('^' || $1 || '-[0-9]+$'))
         AND id IS DISTINCT FROM $2",
        base,
        except,
    )
    .fetch_all(conn)
    .await?;
    Ok(slug::first_free(base, &taken))
}

/// Every News Post, Drafts and Published, the latest changed first.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`.
#[utoipa::path(
    get,
    path = "/news-posts",
    operation_id = "listNewsPosts",
    tag = "content",
    responses(
        (status = OK, description = "Every News Post", body = Vec<NewsPostSummary>),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
    ),
)]
pub(crate) async fn list(
    State(pool): State<PgPool>,
    session: Session,
) -> Result<Json<Vec<NewsPostSummary>>, ApiError> {
    ensure_writer(&session)?;
    let posts = sqlx::query_as!(
        NewsPostSummary,
        "SELECT news_posts.id, title, slug, categories.name AS \"category?\", published,
         first_published_at, updated_at, users.username AS author
         FROM news_posts JOIN users ON users.id = author_id
         LEFT JOIN categories ON categories.id = category_id
         ORDER BY updated_at DESC, news_posts.id DESC"
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(posts))
}

/// One News Post, Draft or Published, for editing.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `404`
/// for an unknown News Post.
#[utoipa::path(
    get,
    path = "/news-posts/{id}",
    operation_id = "getNewsPost",
    tag = "content",
    params(("id" = Uuid, Path, description = "The News Post")),
    responses(
        (status = OK, description = "The News Post", body = NewsPost),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
        (status = NOT_FOUND, description = "There is no such News Post"),
    ),
)]
pub(crate) async fn get(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<Json<NewsPost>, ApiError> {
    ensure_writer(&session)?;
    let row = sqlx::query!(
        "SELECT title, slug, category_id, summary, cover_file_id, cover_caption, cover_alt, body,
         published, first_published_at, updated_at FROM news_posts WHERE id = $1",
        id
    )
    .fetch_optional(&pool)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    Ok(Json(NewsPost {
        id,
        fields: NewsPostFields {
            title: row.title,
            slug: Some(row.slug),
            category: row.category_id,
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

/// Starts a Draft written by the signed-in User; only the title is required.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `409`
/// when another News Post takes the slug meanwhile, `422` for a blank title, a malformed
/// field, a body outside the allowlist, or an unknown Category or cover.
#[utoipa::path(
    post,
    path = "/news-posts",
    operation_id = "createNewsPost",
    tag = "content",
    request_body = NewsPostFields,
    responses(
        (status = CREATED, description = "The new Draft", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
        (status = CONFLICT, description = "Another News Post took the slug meanwhile"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed or unknown"),
    ),
)]
pub(crate) async fn create(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<NewsPostFields>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure_writer(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let base = new
        .slug
        .clone()
        .unwrap_or_else(|| slug::of(&new.title, "berita"));
    let slug = free_slug(&mut tx, &base, None).await?;
    let id = saved(
        sqlx::query_scalar!(
            "INSERT INTO news_posts (title, slug, category_id, summary, cover_file_id,
             cover_caption, cover_alt, body, reading_minutes, author_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
            new.title,
            slug,
            new.category,
            new.summary,
            new.cover,
            new.cover_caption,
            new.cover_alt,
            serde_json::to_value(&new.body)?,
            new.body.reading_minutes(),
            session.user_id,
        )
        .fetch_one(&mut *tx)
        .await,
    )?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "news_post.create",
        ("news_post", Some(&id.to_string())),
        diff(&Value::Null, &new.audited(&slug)).into(),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

/// Saves a News Post: a Draft as it autosaves, a Published one live.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` (or `content.publish`) for a
/// Draft or without `content.publish` for a Published post, `404` for an unknown News Post,
/// `409` for a new slug once first published or one another News Post takes meanwhile, `422`
/// for a blank title, a malformed field, a body outside the allowlist, an unknown Category
/// or cover, or a Published post missing what publishing requires.
#[utoipa::path(
    put,
    path = "/news-posts/{id}",
    operation_id = "updateNewsPost",
    tag = "content",
    params(("id" = Uuid, Path, description = "The News Post")),
    request_body = NewsPostFields,
    responses(
        (status = NO_CONTENT, description = "The News Post is saved"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not change this News Post"),
        (status = NOT_FOUND, description = "There is no such News Post"),
        (status = CONFLICT, description = "The slug is locked or taken"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed, unknown or missing"),
    ),
)]
pub(crate) async fn update(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<NewsPostFields>,
) -> Result<StatusCode, ApiError> {
    ensure_writer(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        "SELECT title, slug, category_id, summary, cover_file_id, cover_caption, cover_alt, body,
         published, first_published_at FROM news_posts WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if old.published {
        ensure_publisher(&session)?;
    }
    let slug = if old.first_published_at.is_some() {
        if new.slug.as_ref().is_some_and(|slug| *slug != old.slug) {
            return Err(ApiError::Status(StatusCode::CONFLICT));
        }
        old.slug.clone()
    } else {
        let base = new
            .slug
            .clone()
            .unwrap_or_else(|| slug::of(&new.title, "berita"));
        free_slug(&mut tx, &base, Some(id)).await?
    };
    let result = sqlx::query!(
        "UPDATE news_posts SET title = $1, slug = $2, category_id = $3, summary = $4,
         cover_file_id = $5, cover_caption = $6, cover_alt = $7, body = $8,
         reading_minutes = $9, updated_at = now() WHERE id = $10",
        new.title,
        slug,
        new.category,
        new.summary,
        new.cover,
        new.cover_caption,
        new.cover_alt,
        serde_json::to_value(&new.body)?,
        new.body.reading_minutes(),
        id,
    )
    .execute(&mut *tx)
    .await;
    match result {
        // A Published post lost what publishing requires.
        Err(sqlx::Error::Database(error)) if error.is_check_violation() => {
            return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
        }
        result => saved(result)?,
    };
    if let Some(cover) = old.cover_file_id.filter(|&cover| Some(cover) != new.cover) {
        storage::release(&mut tx, cover).await?;
    }
    let from = NewsPostFields {
        title: old.title,
        slug: None,
        category: old.category_id,
        summary: old.summary,
        cover: old.cover_file_id,
        cover_caption: old.cover_caption,
        cover_alt: old.cover_alt,
        body: serde_json::from_value(old.body)?,
    }
    .audited(&old.slug);
    let changes = diff(&from, &new.audited(&slug));
    if !changes.is_empty() {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "news_post.update",
            ("news_post", Some(&id.to_string())),
            changes.into(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Puts a Draft on the public site; the first publication fixes its date and locks its slug.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown News
/// Post, `409` when it is already Published, `422` when it lacks a Category, summary, cover
/// or the cover's alt text.
#[utoipa::path(
    post,
    path = "/news-posts/{id}/publish",
    operation_id = "publishNewsPost",
    tag = "content",
    params(("id" = Uuid, Path, description = "The News Post")),
    responses(
        (status = NO_CONTENT, description = "The News Post is Published"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such News Post"),
        (status = CONFLICT, description = "The News Post is already Published"),
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

/// Takes a Published post off the public site, back to Draft; it keeps its date and slug.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown News
/// Post, `409` when it is a Draft.
#[utoipa::path(
    post,
    path = "/news-posts/{id}/unpublish",
    operation_id = "unpublishNewsPost",
    tag = "content",
    params(("id" = Uuid, Path, description = "The News Post")),
    responses(
        (status = NO_CONTENT, description = "The News Post is a Draft again"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such News Post"),
        (status = CONFLICT, description = "The News Post is a Draft"),
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
        "SELECT published FROM news_posts WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if was == published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    let result = sqlx::query!(
        "UPDATE news_posts SET published = $1,
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
            "news_post.publish"
        } else {
            "news_post.unpublish"
        },
        ("news_post", Some(&id.to_string())),
        json!([{ "field": "published", "from": was, "to": published }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a Draft for good; its slug is free again. A Published post is unpublished first.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown News
/// Post, `409` when it is Published.
#[utoipa::path(
    delete,
    path = "/news-posts/{id}",
    operation_id = "deleteNewsPost",
    tag = "content",
    params(("id" = Uuid, Path, description = "The News Post")),
    responses(
        (status = NO_CONTENT, description = "The Draft is deleted"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such News Post"),
        (status = CONFLICT, description = "The News Post is Published"),
    ),
)]
pub(crate) async fn delete(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    ensure_publisher(&session)?;
    let mut tx = pool.begin().await?;
    let published = sqlx::query_scalar!(
        "SELECT published FROM news_posts WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    let old = sqlx::query!(
        "DELETE FROM news_posts WHERE id = $1 RETURNING title, cover_file_id",
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
        "news_post.delete",
        ("news_post", Some(&id.to_string())),
        json!([{ "field": "title", "from": old.title, "to": null }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Stores a cover photo, a JPEG, PNG or WebP of at most 10 MiB, for saving into a News Post.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `413`
/// over 10 MiB, `415` for content that is not JPEG, PNG or WebP, `422` over 40 megapixels or
/// undecodable.
#[utoipa::path(
    post,
    path = "/news-posts/cover",
    operation_id = "uploadNewsPostCover",
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
    State(pool): State<PgPool>,
    State(storage): State<Arc<Storage>>,
    session: Session,
    body: Body,
) -> Result<(StatusCode, Json<Upload>), ApiError> {
    ensure_writer(&session)?;
    let upload = axum::body::to_bytes(body, Kind::Photo.max_bytes())
        .await
        .map_err(|_| ApiError::Status(StatusCode::PAYLOAD_TOO_LARGE))?;
    let id = storage
        .save_image(&pool, Kind::Photo, upload.into())
        .await?;
    Ok((StatusCode::CREATED, Json(Upload { id })))
}

/// How many Published News Posts a page of the public list holds.
const PAGE_SIZE: i64 = 9;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct NewsFilter {
    /// Only News Posts filed under this Category.
    category: Option<Uuid>,
    /// From 1, the default.
    page: Option<u32>,
}

/// A Published News Post in the public list.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewsCard {
    slug: String,
    title: String,
    summary: String,
    /// The Category's name.
    category: String,
    cover: Uuid,
    cover_alt: String,
    /// The first publication.
    published_at: DateTime<Utc>,
    reading_minutes: i32,
}

/// One page of the public News Post list.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewsPage {
    posts: Vec<NewsCard>,
    /// How many Published News Posts match, over every page.
    total: i64,
    page_size: i64,
}

/// Published News Posts, the first published first, a page at a time.
#[utoipa::path(
    get,
    path = "/news-posts/published",
    operation_id = "listPublishedNewsPosts",
    tag = "content",
    params(NewsFilter),
    responses((status = OK, description = "A page of Published News Posts", body = NewsPage)),
)]
pub(crate) async fn list_published(
    State(pool): State<PgPool>,
    Query(filter): Query<NewsFilter>,
) -> Result<Json<NewsPage>, ApiError> {
    let offset = (i64::from(filter.page.unwrap_or(1).max(1)) - 1) * PAGE_SIZE;
    let total = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM news_posts
           WHERE published AND ($1::uuid IS NULL OR category_id = $1)"#,
        filter.category,
    )
    .fetch_one(&pool)
    .await?;
    let posts = sqlx::query_as!(
        NewsCard,
        r#"SELECT slug, title, summary AS "summary!", categories.name AS category,
           cover_file_id AS "cover!", cover_alt AS "cover_alt!",
           first_published_at AS "published_at!", reading_minutes
           FROM news_posts JOIN categories ON categories.id = category_id
           WHERE published AND ($1::uuid IS NULL OR category_id = $1)
           ORDER BY first_published_at DESC, news_posts.id DESC LIMIT $2 OFFSET $3"#,
        filter.category,
        PAGE_SIZE,
        offset,
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(NewsPage {
        posts,
        total,
        page_size: PAGE_SIZE,
    }))
}

/// Who wrote a News Post, as the public sees them.
#[derive(Serialize, ToSchema)]
pub struct Author {
    /// The linked Official's name, or else the User's username.
    name: String,
    /// The linked Official's Position today, if any.
    position: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct CategoryRef {
    id: Uuid,
    name: String,
}

/// A Published News Post as the public reads it.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishedNewsPost {
    slug: String,
    title: String,
    summary: String,
    category: CategoryRef,
    cover: Uuid,
    cover_caption: Option<String>,
    cover_alt: String,
    /// Tiptap JSON of the allowed nodes and marks only.
    #[schema(value_type = Object)]
    body: Value,
    reading_minutes: i32,
    /// The first publication.
    published_at: DateTime<Utc>,
    author: Author,
}

/// One Published News Post by its slug.
///
/// # Errors
/// `404` for a slug no Published News Post has, such as a Draft's or a deleted one's.
#[utoipa::path(
    get,
    path = "/news-posts/published/{slug}",
    operation_id = "getPublishedNewsPost",
    tag = "content",
    params(("slug" = String, Path, description = "The News Post's slug")),
    responses(
        (status = OK, description = "The News Post", body = PublishedNewsPost),
        (status = NOT_FOUND, description = "No Published News Post has the slug"),
    ),
)]
pub(crate) async fn get_published(
    State(pool): State<PgPool>,
    Path(slug): Path<String>,
) -> Result<Json<PublishedNewsPost>, ApiError> {
    // shortcut: "today" is the database's date, not the Village's, so the Position can be a
    // day off around midnight; upgrade if it matters.
    let row = sqlx::query!(
        r#"SELECT slug, title, summary AS "summary!", category_id AS "category_id!",
           categories.name AS category, cover_file_id AS "cover!", cover_caption,
           cover_alt AS "cover_alt!", body, reading_minutes,
           first_published_at AS "published_at!",
           coalesce(officials.name, users.username) AS "author!",
           (SELECT coalesce(positions.name,
                'Kepala ' || initcap(village.hamlet_label) || ' ' || hamlets.name)
            FROM tenures JOIN positions ON positions.id = position_id
            LEFT JOIN region_units hamlets ON hamlets.id = hamlet_id
            LEFT JOIN village ON true
            WHERE tenures.official_id = users.official_id AND start_date <= current_date
            AND (end_date IS NULL OR end_date >= current_date)
            ORDER BY start_date DESC LIMIT 1) AS position
           FROM news_posts JOIN categories ON categories.id = category_id
           JOIN users ON users.id = author_id
           LEFT JOIN officials ON officials.id = users.official_id
           WHERE published AND slug = $1"#,
        slug,
    )
    .fetch_optional(&pool)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    Ok(Json(PublishedNewsPost {
        slug: row.slug,
        title: row.title,
        summary: row.summary,
        category: CategoryRef {
            id: row.category_id,
            name: row.category,
        },
        cover: row.cover,
        cover_caption: row.cover_caption,
        cover_alt: row.cover_alt,
        body: row.body,
        reading_minutes: row.reading_minutes,
        published_at: row.published_at,
        author: Author {
            name: row.author,
            position: row.position,
        },
    }))
}
