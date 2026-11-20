//! Albums: ordered photos from one village occasion, written as Drafts and then Published.
//!
//! `content.write` creates and edits Drafts; `content.publish` also publishes, unpublishes,
//! edits Published Albums and deletes Drafts.

use std::{collections::HashSet, sync::Arc};

use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::{DateTime, NaiveDate, Utc};
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
    news_post::{self, ensure_writer, saved},
    slug,
    storage::{self, Storage},
    village::{Upload, diff},
};

/// One photo of an Album.
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct AlbumPhoto {
    /// Uploaded at `/albums/photo`.
    file: Uuid,
    /// Up to 300 characters.
    #[serde(default)]
    caption: Option<String>,
    /// Up to 300 characters; required to publish.
    #[serde(default)]
    alt: Option<String>,
}

/// What a User writes in an Album. A Draft may leave out what publishing requires.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AlbumFields {
    /// 1–150 characters.
    title: String,
    /// Made from the title when blank; transliterated, and locked once first published.
    #[serde(default)]
    slug: Option<String>,
    /// Up to 500 characters.
    #[serde(default)]
    description: Option<String>,
    /// The occasion's first day; required to publish.
    #[serde(default)]
    occasion_start: Option<NaiveDate>,
    /// The occasion's last day, when it spans several; not before the start.
    #[serde(default)]
    occasion_end: Option<NaiveDate>,
    /// In the order the public sees, each file once; publishing needs at least one.
    #[serde(default)]
    photos: Vec<AlbumPhoto>,
    /// One of the photos' files; null makes the first photo the cover.
    #[serde(default)]
    cover: Option<Uuid>,
}

impl AlbumFields {
    /// Trims every text, blank to null, and refuses with `422` what is malformed.
    fn normalized(self) -> Result<Self, ApiError> {
        let invalid = || ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY);
        let text = |value: Option<String>, max: usize| match value.as_deref().map(str::trim) {
            None | Some("") => Ok(None),
            Some(value) if value.chars().count() <= max => Ok(Some(value.to_owned())),
            Some(_) => Err(invalid()),
        };
        if let (Some(start), Some(end)) = (self.occasion_start, self.occasion_end)
            && end < start
        {
            return Err(invalid());
        }
        let mut files = HashSet::new();
        if !self.photos.iter().all(|photo| files.insert(photo.file))
            || self.cover.is_some_and(|cover| !files.contains(&cover))
        {
            return Err(invalid());
        }
        Ok(Self {
            title: text(Some(self.title), 150)?.ok_or_else(invalid)?,
            slug: self
                .slug
                .map(|slug| slug::of(&slug, ""))
                .filter(|slug| !slug.is_empty()),
            description: text(self.description, 500)?,
            occasion_start: self.occasion_start,
            occasion_end: self.occasion_end,
            photos: self
                .photos
                .into_iter()
                .map(|photo| {
                    Ok(AlbumPhoto {
                        file: photo.file,
                        caption: text(photo.caption, 300)?,
                        alt: text(photo.alt, 300)?,
                    })
                })
                .collect::<Result<_, ApiError>>()?,
            cover: self.cover,
        })
    }

    /// The fields as an Audit Entry shows them.
    fn audited(&self, slug: &str) -> Value {
        json!({
            "title": self.title, "slug": slug, "description": self.description,
            "occasionStart": self.occasion_start, "occasionEnd": self.occasion_end,
            "photos": self.photos, "cover": self.cover,
        })
    }

    /// Replaces the Album's photos with these, in order.
    async fn save_photos(&self, conn: &mut sqlx::PgConnection, id: Uuid) -> Result<(), ApiError> {
        sqlx::query!("DELETE FROM album_photos WHERE album_id = $1", id)
            .execute(&mut *conn)
            .await?;
        let files: Vec<Uuid> = self.photos.iter().map(|photo| photo.file).collect();
        let captions: Vec<Option<String>> = self.photos.iter().map(|p| p.caption.clone()).collect();
        let alts: Vec<Option<String>> = self.photos.iter().map(|p| p.alt.clone()).collect();
        saved(
            sqlx::query!(
                "INSERT INTO album_photos (album_id, position, file_id, caption, alt)
                 SELECT $1, (ord - 1)::integer, file, caption, alt
                 FROM unnest($2::uuid[], $3::text[], $4::text[])
                     WITH ORDINALITY AS photo (file, caption, alt, ord)",
                id,
                &files,
                &captions as &[Option<String>],
                &alts as &[Option<String>],
            )
            .execute(conn)
            .await,
        )?;
        Ok(())
    }
}

/// Whether the photos are what publishing requires: at least one, each with alt text.
fn publishable(photos: &[AlbumPhoto]) -> bool {
    !photos.is_empty() && photos.iter().all(|photo| photo.alt.is_some())
}

/// An Album's photos in order.
async fn photos(conn: impl sqlx::PgExecutor<'_>, id: Uuid) -> sqlx::Result<Vec<AlbumPhoto>> {
    sqlx::query_as!(
        AlbumPhoto,
        "SELECT file_id AS file, caption, alt FROM album_photos
         WHERE album_id = $1 ORDER BY position",
        id
    )
    .fetch_all(conn)
    .await
}

/// An Album as the admin area edits it.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    id: Uuid,
    #[serde(flatten)]
    fields: AlbumFields,
    published: bool,
    /// Null until first published; the slug is locked once set.
    first_published_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

/// A line of the admin area's Album list.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AlbumSummary {
    id: Uuid,
    title: String,
    slug: String,
    occasion_start: Option<NaiveDate>,
    photo_count: i64,
    published: bool,
    updated_at: DateTime<Utc>,
}

/// `base`, or `base-2`, `base-3`… when another Album than `except` has it.
async fn free_slug(
    conn: &mut sqlx::PgConnection,
    base: &str,
    except: Option<Uuid>,
) -> sqlx::Result<String> {
    // `base` is a slug, so it holds nothing a regular expression would read.
    let taken = sqlx::query_scalar!(
        "SELECT slug FROM albums WHERE (slug = $1 OR slug ~ ('^' || $1 || '-[0-9]+$'))
         AND id IS DISTINCT FROM $2",
        base,
        except,
    )
    .fetch_all(conn)
    .await?;
    Ok(slug::first_free(base, &taken))
}

/// Every Album, Drafts and Published, the latest occasion first and undated Drafts on top.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`.
#[utoipa::path(
    get,
    path = "/albums",
    operation_id = "listAlbums",
    tag = "content",
    responses(
        (status = OK, description = "Every Album", body = Vec<AlbumSummary>),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
    ),
)]
pub(crate) async fn list(
    State(pool): State<PgPool>,
    session: Session,
) -> Result<Json<Vec<AlbumSummary>>, ApiError> {
    ensure_writer(&session)?;
    let albums = sqlx::query_as!(
        AlbumSummary,
        r#"SELECT id, title, slug, occasion_start, published, updated_at,
           (SELECT count(*) FROM album_photos WHERE album_id = albums.id) AS "photo_count!"
           FROM albums ORDER BY occasion_start DESC NULLS FIRST, updated_at DESC, id"#
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(albums))
}

/// One Album, Draft or Published, for editing.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `404`
/// for an unknown Album.
#[utoipa::path(
    get,
    path = "/albums/{id}",
    operation_id = "getAlbum",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Album")),
    responses(
        (status = OK, description = "The Album", body = Album),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
        (status = NOT_FOUND, description = "There is no such Album"),
    ),
)]
pub(crate) async fn get(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<Json<Album>, ApiError> {
    ensure_writer(&session)?;
    let row = sqlx::query!(
        "SELECT title, slug, description, occasion_start, occasion_end, cover_file_id,
         published, first_published_at, updated_at FROM albums WHERE id = $1",
        id
    )
    .fetch_optional(&pool)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    Ok(Json(Album {
        id,
        fields: AlbumFields {
            title: row.title,
            slug: Some(row.slug),
            description: row.description,
            occasion_start: row.occasion_start,
            occasion_end: row.occasion_end,
            photos: photos(&pool, id).await?,
            cover: row.cover_file_id,
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
/// when another Album takes the slug meanwhile, `422` for a blank title, a malformed field, an
/// end before the start, a photo listed twice or unknown, or a cover that is not a photo.
#[utoipa::path(
    post,
    path = "/albums",
    operation_id = "createAlbum",
    tag = "content",
    request_body = AlbumFields,
    responses(
        (status = CREATED, description = "The new Draft", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
        (status = CONFLICT, description = "Another Album took the slug meanwhile"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed or unknown"),
    ),
)]
pub(crate) async fn create(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<AlbumFields>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure_writer(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let base = new
        .slug
        .clone()
        .unwrap_or_else(|| slug::of(&new.title, "album"));
    let slug = free_slug(&mut tx, &base, None).await?;
    let id = saved(
        sqlx::query_scalar!(
            "INSERT INTO albums (title, slug, description, occasion_start, occasion_end,
             cover_file_id, author_id) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
            new.title,
            slug,
            new.description,
            new.occasion_start,
            new.occasion_end,
            new.cover,
            session.user_id,
        )
        .fetch_one(&mut *tx)
        .await,
    )?;
    new.save_photos(&mut tx, id).await?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "album.create",
        ("album", Some(&id.to_string())),
        diff(&Value::Null, &new.audited(&slug)).into(),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

/// Saves an Album: a Draft as it autosaves, a Published one live.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` (or `content.publish`) for a
/// Draft or without `content.publish` for a Published Album, `404` for an unknown Album, `409`
/// for a new slug once first published or a slug another Album takes meanwhile, `422` for a
/// blank title, a malformed field, an end before the start, a photo listed twice or unknown, a
/// cover that is not a photo, or a Published Album missing what publishing requires.
#[utoipa::path(
    put,
    path = "/albums/{id}",
    operation_id = "updateAlbum",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Album")),
    request_body = AlbumFields,
    responses(
        (status = NO_CONTENT, description = "The Album is saved"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not change this Album"),
        (status = NOT_FOUND, description = "There is no such Album"),
        (status = CONFLICT, description = "The slug is locked or taken"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed, unknown or missing"),
    ),
)]
pub(crate) async fn update(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<AlbumFields>,
) -> Result<StatusCode, ApiError> {
    ensure_writer(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        "SELECT title, slug, description, occasion_start, occasion_end, cover_file_id,
         published, first_published_at FROM albums WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if old.published {
        ensure_publisher(&session)?;
        if !publishable(&new.photos) {
            return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
        }
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
            .unwrap_or_else(|| slug::of(&new.title, "album"));
        free_slug(&mut tx, &base, Some(id)).await?
    };
    let result = sqlx::query!(
        "UPDATE albums SET title = $1, slug = $2, description = $3, occasion_start = $4,
         occasion_end = $5, cover_file_id = $6, updated_at = now() WHERE id = $7",
        new.title,
        slug,
        new.description,
        new.occasion_start,
        new.occasion_end,
        new.cover,
        id,
    )
    .execute(&mut *tx)
    .await;
    match result {
        // A Published Album lost what publishing requires.
        Err(sqlx::Error::Database(error)) if error.is_check_violation() => {
            return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
        }
        result => saved(result)?,
    };
    let old_photos = photos(&mut *tx, id).await?;
    new.save_photos(&mut tx, id).await?;
    for photo in &old_photos {
        if !new.photos.iter().any(|kept| kept.file == photo.file) {
            storage::release(&mut tx, photo.file).await?;
        }
    }
    let from = AlbumFields {
        title: old.title,
        slug: None,
        description: old.description,
        occasion_start: old.occasion_start,
        occasion_end: old.occasion_end,
        photos: old_photos,
        cover: old.cover_file_id,
    }
    .audited(&old.slug);
    let changes = diff(&from, &new.audited(&slug));
    if !changes.is_empty() {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "album.update",
            ("album", Some(&id.to_string())),
            changes.into(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Puts a Draft on the public site; the first publication locks its slug.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown Album,
/// `409` when it is already Published, `422` when it has no photos, a photo without alt text,
/// or no occasion start.
#[utoipa::path(
    post,
    path = "/albums/{id}/publish",
    operation_id = "publishAlbum",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Album")),
    responses(
        (status = NO_CONTENT, description = "The Album is Published"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Album"),
        (status = CONFLICT, description = "The Album is already Published"),
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

/// Takes a Published Album off the public site, back to Draft; it keeps its slug.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown Album,
/// `409` when it is a Draft.
#[utoipa::path(
    post,
    path = "/albums/{id}/unpublish",
    operation_id = "unpublishAlbum",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Album")),
    responses(
        (status = NO_CONTENT, description = "The Album is a Draft again"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Album"),
        (status = CONFLICT, description = "The Album is a Draft"),
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
    let was = sqlx::query_scalar!("SELECT published FROM albums WHERE id = $1 FOR UPDATE", id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if was == published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    if published && !publishable(&photos(&mut *tx, id).await?) {
        return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
    }
    let result = sqlx::query!(
        "UPDATE albums SET published = $1,
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
            "album.publish"
        } else {
            "album.unpublish"
        },
        ("album", Some(&id.to_string())),
        json!([{ "field": "published", "from": was, "to": published }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a Draft for good; its slug is free again. A Published Album is unpublished first.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown Album,
/// `409` when it is Published.
#[utoipa::path(
    delete,
    path = "/albums/{id}",
    operation_id = "deleteAlbum",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Album")),
    responses(
        (status = NO_CONTENT, description = "The Draft is deleted"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Album"),
        (status = CONFLICT, description = "The Album is Published"),
    ),
)]
pub(crate) async fn delete(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    ensure_publisher(&session)?;
    let mut tx = pool.begin().await?;
    let published =
        sqlx::query_scalar!("SELECT published FROM albums WHERE id = $1 FOR UPDATE", id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    let photos = photos(&mut *tx, id).await?;
    // The photos go with the Album.
    let title = sqlx::query_scalar!("DELETE FROM albums WHERE id = $1 RETURNING title", id)
        .fetch_one(&mut *tx)
        .await?;
    for photo in photos {
        storage::release(&mut tx, photo.file).await?;
    }
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "album.delete",
        ("album", Some(&id.to_string())),
        json!([{ "field": "title", "from": title, "to": null }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Stores a photo, a JPEG, PNG or WebP of at most 10 MiB, for saving into an Album.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `413`
/// over 10 MiB, `415` for content that is not JPEG, PNG or WebP, `422` over 40 megapixels or
/// undecodable.
#[utoipa::path(
    post,
    path = "/albums/photo",
    operation_id = "uploadAlbumPhoto",
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
pub(crate) async fn upload_photo(
    pool: State<PgPool>,
    storage: State<Arc<Storage>>,
    session: Session,
    body: Body,
) -> Result<(StatusCode, Json<Upload>), ApiError> {
    // The same photo rules as a News Post's cover.
    news_post::upload_cover(pool, storage, session, body).await
}

/// How many Published Albums a page of the gallery holds.
const PAGE_SIZE: i64 = 12;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct GalleryFilter {
    /// From 1, the default.
    page: Option<u32>,
}

/// A Published Album in the gallery.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AlbumCard {
    slug: String,
    title: String,
    occasion_start: NaiveDate,
    occasion_end: Option<NaiveDate>,
    cover: Uuid,
    cover_alt: String,
    photo_count: i64,
}

/// One page of the gallery.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GalleryPage {
    albums: Vec<AlbumCard>,
    /// How many Albums are Published, over every page.
    total: i64,
    page_size: i64,
}

/// Published Albums, the latest occasion first, a page at a time.
#[utoipa::path(
    get,
    path = "/albums/published",
    operation_id = "listPublishedAlbums",
    tag = "content",
    params(GalleryFilter),
    responses((status = OK, description = "A page of Published Albums", body = GalleryPage)),
)]
pub(crate) async fn list_published(
    State(pool): State<PgPool>,
    Query(filter): Query<GalleryFilter>,
) -> Result<Json<GalleryPage>, ApiError> {
    let offset = (i64::from(filter.page.unwrap_or(1).max(1)) - 1) * PAGE_SIZE;
    let total = sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM albums WHERE published"#)
        .fetch_one(&pool)
        .await?;
    // A Published Album always has a photo, so the lateral join keeps every one.
    let albums = sqlx::query_as!(
        AlbumCard,
        r#"SELECT slug, title, occasion_start AS "occasion_start!", occasion_end,
           cover.file_id AS "cover!", cover.alt AS "cover_alt!",
           (SELECT count(*) FROM album_photos WHERE album_id = albums.id) AS "photo_count!"
           FROM albums CROSS JOIN LATERAL (
               SELECT file_id, alt FROM album_photos WHERE album_id = albums.id
               ORDER BY file_id IS NOT DISTINCT FROM albums.cover_file_id DESC, position
               LIMIT 1
           ) cover
           WHERE published ORDER BY occasion_start DESC, albums.id DESC LIMIT $1 OFFSET $2"#,
        PAGE_SIZE,
        offset,
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(GalleryPage {
        albums,
        total,
        page_size: PAGE_SIZE,
    }))
}

/// A photo of a Published Album.
#[derive(Serialize, ToSchema)]
pub struct PublishedPhoto {
    file: Uuid,
    caption: Option<String>,
    alt: String,
}

/// A Published Album as the public sees it.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishedAlbum {
    slug: String,
    title: String,
    description: Option<String>,
    occasion_start: NaiveDate,
    occasion_end: Option<NaiveDate>,
    /// The chosen photo, or else the first.
    cover: Uuid,
    photos: Vec<PublishedPhoto>,
}

/// One Published Album by its slug.
///
/// # Errors
/// `404` for a slug no Published Album has, such as a Draft's or a deleted one's.
#[utoipa::path(
    get,
    path = "/albums/published/{slug}",
    operation_id = "getPublishedAlbum",
    tag = "content",
    params(("slug" = String, Path, description = "The Album's slug")),
    responses(
        (status = OK, description = "The Album", body = PublishedAlbum),
        (status = NOT_FOUND, description = "No Published Album has the slug"),
    ),
)]
pub(crate) async fn get_published(
    State(pool): State<PgPool>,
    Path(slug): Path<String>,
) -> Result<Json<PublishedAlbum>, ApiError> {
    let album = sqlx::query!(
        r#"SELECT id, slug, title, description, occasion_start AS "occasion_start!",
           occasion_end, cover_file_id FROM albums WHERE published AND slug = $1"#,
        slug,
    )
    .fetch_optional(&pool)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    let photos = sqlx::query_as!(
        PublishedPhoto,
        r#"SELECT file_id AS file, caption, alt AS "alt!" FROM album_photos
           WHERE album_id = $1 ORDER BY position"#,
        album.id
    )
    .fetch_all(&pool)
    .await?;
    let cover = album
        .cover_file_id
        .or_else(|| photos.first().map(|photo| photo.file))
        .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    Ok(Json(PublishedAlbum {
        slug: album.slug,
        title: album.title,
        description: album.description,
        occasion_start: album.occasion_start,
        occasion_end: album.occasion_end,
        cover,
        photos,
    }))
}
