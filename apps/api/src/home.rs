//! The home page singletons: the hero with its Quick Links, saved live by `content.publish`.

use std::sync::Arc;

use axum::{Json, body::Body, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, types::Json as Jsonb};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{
    ApiError, AppState,
    audit::{self, Actor},
    auth::Session,
    category::ensure_publisher,
    imaging::Kind,
    news_post::saved,
    storage::{self, Storage},
    village::{Upload, diff},
};

pub(crate) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_hero, update_hero))
        .routes(routes!(upload_hero_photo))
}

/// A shortcut under the home page search box.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct QuickLink {
    /// 1–40 characters.
    label: String,
    /// A path on the public site, such as `/berita`.
    path: String,
}

/// The home page hero.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HomeHero {
    /// Uploaded at `/home/hero/photo`; served up to `/media/{photo}/2400`.
    photo: Uuid,
    /// 1–200 characters.
    photo_alt: String,
    /// 1–100 characters.
    title: String,
    /// Up to 200 characters.
    #[serde(default)]
    subtitle: Option<String>,
    /// Up to 40 characters; the button shows only with a label and a path.
    #[serde(default)]
    button_label: Option<String>,
    /// A path on the public site, such as `/profil`.
    #[serde(default)]
    button_path: Option<String>,
    /// At most four, in order.
    #[serde(default)]
    quick_links: Vec<QuickLink>,
}

/// Trims a text, blank to null; `422` when it is longer than `max`.
fn text(value: Option<&str>, max: usize) -> Result<Option<String>, ApiError> {
    match value.map(str::trim) {
        None | Some("") => Ok(None),
        Some(value) if value.chars().count() <= max => Ok(Some(value.to_owned())),
        Some(_) => Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY)),
    }
}

/// [`text`] that may not be blank.
fn required(value: &str, max: usize) -> Result<String, ApiError> {
    text(Some(value), max)?.ok_or(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))
}

/// A path on the public site: one leading `/`, never a `//host` or `/\host` that leaves it.
fn internal_path(path: &str) -> Result<String, ApiError> {
    let path = required(path, 200)?;
    let ok = path.starts_with('/')
        && !path[1..].starts_with(['/', '\\'])
        && !path.chars().any(|c| c.is_whitespace() || c.is_control());
    if ok {
        Ok(path)
    } else {
        Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))
    }
}

impl HomeHero {
    /// Trims every text, blank to null, and refuses with `422` what is malformed.
    fn normalized(self) -> Result<Self, ApiError> {
        let button_label = text(self.button_label.as_deref(), 40)?;
        let button_path = text(self.button_path.as_deref(), 200)?
            .as_deref()
            .map(internal_path)
            .transpose()?;
        if button_label.is_some() != button_path.is_some() || self.quick_links.len() > 4 {
            return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
        }
        Ok(Self {
            photo: self.photo,
            photo_alt: required(&self.photo_alt, 200)?,
            title: required(&self.title, 100)?,
            subtitle: text(self.subtitle.as_deref(), 200)?,
            button_label,
            button_path,
            quick_links: self
                .quick_links
                .iter()
                .map(|link| {
                    Ok(QuickLink {
                        label: required(&link.label, 40)?,
                        path: internal_path(&link.path)?,
                    })
                })
                .collect::<Result<_, ApiError>>()?,
        })
    }
}

async fn hero(conn: &mut sqlx::PgConnection) -> sqlx::Result<Option<HomeHero>> {
    let row = sqlx::query!(
        r#"SELECT photo_file_id, photo_alt, title, subtitle, button_label, button_path,
           quick_links AS "quick_links: Jsonb<Vec<QuickLink>>"
           FROM home_hero"#
    )
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|row| HomeHero {
        photo: row.photo_file_id,
        photo_alt: row.photo_alt,
        title: row.title,
        subtitle: row.subtitle,
        button_label: row.button_label,
        button_path: row.button_path,
        quick_links: row.quick_links.0,
    }))
}

/// The home page hero, for the public site and the admin area alike.
///
/// # Errors
/// `404` before it is first saved.
#[utoipa::path(
    get,
    path = "/home/hero",
    operation_id = "getHomeHero",
    tag = "content",
    responses(
        (status = OK, description = "The hero", body = HomeHero),
        (status = NOT_FOUND, description = "No hero is saved yet"),
    ),
)]
pub(crate) async fn get_hero(State(pool): State<PgPool>) -> Result<Json<HomeHero>, ApiError> {
    hero(&mut *pool.acquire().await?)
        .await?
        .map(Json)
        .ok_or(ApiError::Status(StatusCode::NOT_FOUND))
}

/// Saves the home page hero live.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `422` for a blank or
/// overlong text, an unknown photo, a button with a label or a path alone, a path off the
/// public site, or over four Quick Links.
#[utoipa::path(
    put,
    path = "/home/hero",
    operation_id = "updateHomeHero",
    tag = "content",
    request_body = HomeHero,
    responses(
        (status = NO_CONTENT, description = "The hero is saved"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed or unknown"),
    ),
)]
pub(crate) async fn update_hero(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<HomeHero>,
) -> Result<StatusCode, ApiError> {
    ensure_publisher(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    // One save at a time, the first included, so the audited change is the real one.
    sqlx::query!("LOCK TABLE home_hero IN EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let old = hero(&mut tx).await?;
    saved(
        sqlx::query!(
            "INSERT INTO home_hero (photo_file_id, photo_alt, title, subtitle, button_label,
             button_path, quick_links) VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (singleton) DO UPDATE SET photo_file_id = EXCLUDED.photo_file_id,
             photo_alt = EXCLUDED.photo_alt, title = EXCLUDED.title,
             subtitle = EXCLUDED.subtitle, button_label = EXCLUDED.button_label,
             button_path = EXCLUDED.button_path, quick_links = EXCLUDED.quick_links",
            new.photo,
            new.photo_alt,
            new.title,
            new.subtitle,
            new.button_label,
            new.button_path,
            Jsonb(&new.quick_links) as _,
        )
        .execute(&mut *tx)
        .await,
    )?;
    if let Some(photo) = old
        .as_ref()
        .map(|old| old.photo)
        .filter(|&photo| photo != new.photo)
    {
        storage::release(&mut tx, photo).await?;
    }
    let changes = diff(&serde_json::to_value(&old)?, &serde_json::to_value(&new)?);
    if !changes.is_empty() {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "home.update_hero",
            ("home_hero", None),
            changes.into(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Stores a hero photo, a JPEG, PNG or WebP of at most 10 MiB, with its 2400 px variant.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `413` over 10 MiB, `415`
/// for content that is not JPEG, PNG or WebP, `422` over 40 megapixels or undecodable.
#[utoipa::path(
    post,
    path = "/home/hero/photo",
    operation_id = "uploadHomeHeroPhoto",
    tag = "content",
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = CREATED, description = "The stored photo", body = Upload),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = PAYLOAD_TOO_LARGE, description = "The file is over 10 MiB"),
        (status = UNSUPPORTED_MEDIA_TYPE, description = "The file is not JPEG, PNG or WebP"),
        (status = UNPROCESSABLE_ENTITY, description = "The image is too large or broken"),
    ),
)]
pub(crate) async fn upload_hero_photo(
    State(pool): State<PgPool>,
    State(storage): State<Arc<Storage>>,
    session: Session,
    body: Body,
) -> Result<(StatusCode, Json<Upload>), ApiError> {
    upload(&pool, &storage, &session, body, Kind::Hero).await
}

/// Stores a photo of `kind` for a publisher.
async fn upload(
    pool: &PgPool,
    storage: &Storage,
    session: &Session,
    body: Body,
    kind: Kind,
) -> Result<(StatusCode, Json<Upload>), ApiError> {
    ensure_publisher(session)?;
    let upload = axum::body::to_bytes(body, kind.max_bytes())
        .await
        .map_err(|_| ApiError::Status(StatusCode::PAYLOAD_TOO_LARGE))?;
    let id = storage.save_image(pool, kind, upload.into()).await?;
    Ok((StatusCode::CREATED, Json(Upload { id })))
}
