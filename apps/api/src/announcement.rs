//! Announcements: notices valid from a start to an end date, written as Drafts and then
//! Published. Whether one is in effect is computed from today in the Village's time zone.
//!
//! `content.write` creates and edits Drafts; `content.publish` also publishes, unpublishes,
//! edits Published Announcements and deletes Drafts.

use axum::{
    Json,
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
    ApiError, Clock,
    audit::{self, Actor},
    auth::Session,
    category::ensure_publisher,
    government::Created,
    news_post::ensure_writer,
    rich_text::Doc,
    slug,
    village::{self, diff},
};

/// What a User writes in an Announcement. A Draft may leave out what publishing requires.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementFields {
    /// 1–150 characters.
    title: String,
    /// Made from the title when blank; transliterated, and locked once first published.
    #[serde(default)]
    slug: Option<String>,
    /// Plain text, at most 250 characters; required to publish.
    #[serde(default)]
    summary: Option<String>,
    /// The first day in effect, in the Village's time zone; required to publish.
    #[serde(default)]
    start_date: Option<NaiveDate>,
    /// The last day in effect, inclusive and not before the start; required to publish.
    #[serde(default)]
    end_date: Option<NaiveDate>,
    /// Optional; only an Announcement with a body has a detail page. Tiptap JSON of the same
    /// allowlist as a News Post's body.
    #[serde(default = "Doc::empty")]
    #[schema(value_type = Object)]
    body: Doc,
}

impl AnnouncementFields {
    /// Trims every text, blank to null, empties a blank body, and refuses with `422` what is
    /// malformed.
    fn normalized(self) -> Result<Self, ApiError> {
        let text = |value: Option<String>, max: usize| match value.as_deref().map(str::trim) {
            None | Some("") => Ok(None),
            Some(value) if value.chars().count() <= max => Ok(Some(value.to_owned())),
            Some(_) => Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY)),
        };
        if let (Some(start), Some(end)) = (self.start_date, self.end_date)
            && end < start
        {
            return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
        }
        Ok(Self {
            title: text(Some(self.title), 150)?
                .ok_or(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))?,
            slug: self
                .slug
                .map(|slug| slug::of(&slug, ""))
                .filter(|slug| !slug.is_empty()),
            summary: text(self.summary, 250)?,
            start_date: self.start_date,
            end_date: self.end_date,
            body: if self.body.is_blank() {
                Doc::empty()
            } else {
                self.body
            },
        })
    }

    /// The fields as an Audit Entry shows them, the body as plain text.
    fn audited(&self, slug: &str) -> Value {
        json!({
            "title": self.title, "slug": slug, "summary": self.summary,
            "startDate": self.start_date, "endDate": self.end_date,
            "body": self.body.plain_text(),
        })
    }
}

/// Berlaku, Akan berlaku or Berakhir, on a given day.
#[derive(Serialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Validity {
    Upcoming,
    InEffect,
    Ended,
}

impl Validity {
    fn on(today: NaiveDate, start: NaiveDate, end: NaiveDate) -> Self {
        if today < start {
            Self::Upcoming
        } else if today > end {
            Self::Ended
        } else {
            Self::InEffect
        }
    }
}

/// An Announcement as the admin area edits it.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Announcement {
    id: Uuid,
    #[serde(flatten)]
    fields: AnnouncementFields,
    published: bool,
    /// Null until first published; the slug is locked once set.
    first_published_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

/// A line of the admin area's Announcement list.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementSummary {
    id: Uuid,
    title: String,
    slug: String,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
    /// Today's, once both dates are set.
    validity: Option<Validity>,
    published: bool,
    first_published_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
    /// The creating User's username.
    author: String,
}

/// An unknown author says the body is wrong; a slug taken meanwhile, a conflict; a Published
/// Announcement missing what publishing requires, a `422`.
fn saved<T>(result: sqlx::Result<T>) -> Result<T, ApiError> {
    match result {
        Err(sqlx::Error::Database(error))
            if error.is_foreign_key_violation() || error.is_check_violation() =>
        {
            Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))
        }
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(ApiError::Status(StatusCode::CONFLICT))
        }
        result => Ok(result?),
    }
}

/// `base`, or `base-2`, `base-3`… when another Announcement than `except` has it.
async fn free_slug(
    conn: &mut sqlx::PgConnection,
    base: &str,
    except: Option<Uuid>,
) -> sqlx::Result<String> {
    // `base` is a slug, so it holds nothing a regular expression would read.
    let taken = sqlx::query_scalar!(
        "SELECT slug FROM announcements WHERE (slug = $1 OR slug ~ ('^' || $1 || '-[0-9]+$'))
         AND id IS DISTINCT FROM $2",
        base,
        except,
    )
    .fetch_all(conn)
    .await?;
    Ok(slug::first_free(base, &taken))
}

/// Every Announcement, Drafts and Published, the latest changed first.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`.
#[utoipa::path(
    get,
    path = "/announcements",
    operation_id = "listAnnouncements",
    tag = "content",
    responses(
        (status = OK, description = "Every Announcement", body = Vec<AnnouncementSummary>),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
    ),
)]
pub(crate) async fn list(
    State(pool): State<PgPool>,
    State(clock): State<Clock>,
    session: Session,
) -> Result<Json<Vec<AnnouncementSummary>>, ApiError> {
    ensure_writer(&session)?;
    let today = village::today(&pool, clock.now()).await?;
    let rows = sqlx::query!(
        "SELECT announcements.id, title, slug, start_date, end_date, published,
         first_published_at, updated_at, users.username AS author
         FROM announcements JOIN users ON users.id = author_id
         ORDER BY updated_at DESC, announcements.id DESC"
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| AnnouncementSummary {
                id: row.id,
                title: row.title,
                slug: row.slug,
                start_date: row.start_date,
                end_date: row.end_date,
                validity: row
                    .start_date
                    .zip(row.end_date)
                    .map(|(start, end)| Validity::on(today, start, end)),
                published: row.published,
                first_published_at: row.first_published_at,
                updated_at: row.updated_at,
                author: row.author,
            })
            .collect(),
    ))
}

/// One Announcement, Draft or Published, for editing.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `404`
/// for an unknown Announcement.
#[utoipa::path(
    get,
    path = "/announcements/{id}",
    operation_id = "getAnnouncement",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Announcement")),
    responses(
        (status = OK, description = "The Announcement", body = Announcement),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
        (status = NOT_FOUND, description = "There is no such Announcement"),
    ),
)]
pub(crate) async fn get(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<Json<Announcement>, ApiError> {
    ensure_writer(&session)?;
    let row = sqlx::query!(
        "SELECT title, slug, summary, start_date, end_date, body, published,
         first_published_at, updated_at FROM announcements WHERE id = $1",
        id
    )
    .fetch_optional(&pool)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    Ok(Json(Announcement {
        id,
        fields: AnnouncementFields {
            title: row.title,
            slug: Some(row.slug),
            summary: row.summary,
            start_date: row.start_date,
            end_date: row.end_date,
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
/// when another Announcement takes the slug meanwhile, `422` for a blank title, a malformed
/// field, an end before the start or a body outside the allowlist.
#[utoipa::path(
    post,
    path = "/announcements",
    operation_id = "createAnnouncement",
    tag = "content",
    request_body = AnnouncementFields,
    responses(
        (status = CREATED, description = "The new Draft", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
        (status = CONFLICT, description = "Another Announcement took the slug meanwhile"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed"),
    ),
)]
pub(crate) async fn create(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<AnnouncementFields>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure_writer(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let base = new
        .slug
        .clone()
        .unwrap_or_else(|| slug::of(&new.title, "pengumuman"));
    let slug = free_slug(&mut tx, &base, None).await?;
    let id = saved(
        sqlx::query_scalar!(
            "INSERT INTO announcements (title, slug, summary, start_date, end_date, body,
             author_id) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
            new.title,
            slug,
            new.summary,
            new.start_date,
            new.end_date,
            serde_json::to_value(&new.body)?,
            session.user_id,
        )
        .fetch_one(&mut *tx)
        .await,
    )?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "announcement.create",
        ("announcement", Some(&id.to_string())),
        diff(&Value::Null, &new.audited(&slug)).into(),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

/// Saves an Announcement: a Draft as it autosaves, a Published one live.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` (or `content.publish`) for a
/// Draft or without `content.publish` for a Published Announcement, `404` for an unknown
/// Announcement, `409` for a new slug once first published or one another Announcement takes
/// meanwhile, `422` for a blank title, a malformed field, an end before the start, a body
/// outside the allowlist, or a Published Announcement missing what publishing requires.
#[utoipa::path(
    put,
    path = "/announcements/{id}",
    operation_id = "updateAnnouncement",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Announcement")),
    request_body = AnnouncementFields,
    responses(
        (status = NO_CONTENT, description = "The Announcement is saved"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not change this Announcement"),
        (status = NOT_FOUND, description = "There is no such Announcement"),
        (status = CONFLICT, description = "The slug is locked or taken"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed or missing"),
    ),
)]
pub(crate) async fn update(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<AnnouncementFields>,
) -> Result<StatusCode, ApiError> {
    ensure_writer(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        "SELECT title, slug, summary, start_date, end_date, body, published, first_published_at
         FROM announcements WHERE id = $1 FOR UPDATE",
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
            .unwrap_or_else(|| slug::of(&new.title, "pengumuman"));
        free_slug(&mut tx, &base, Some(id)).await?
    };
    saved(
        sqlx::query!(
            "UPDATE announcements SET title = $1, slug = $2, summary = $3, start_date = $4,
             end_date = $5, body = $6, updated_at = now() WHERE id = $7",
            new.title,
            slug,
            new.summary,
            new.start_date,
            new.end_date,
            serde_json::to_value(&new.body)?,
            id,
        )
        .execute(&mut *tx)
        .await,
    )?;
    let from = AnnouncementFields {
        title: old.title,
        slug: None,
        summary: old.summary,
        start_date: old.start_date,
        end_date: old.end_date,
        body: serde_json::from_value(old.body)?,
    }
    .audited(&old.slug);
    let changes = diff(&from, &new.audited(&slug));
    if !changes.is_empty() {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "announcement.update",
            ("announcement", Some(&id.to_string())),
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
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// Announcement, `409` when it is already Published, `422` when it lacks a summary or a date.
#[utoipa::path(
    post,
    path = "/announcements/{id}/publish",
    operation_id = "publishAnnouncement",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Announcement")),
    responses(
        (status = NO_CONTENT, description = "The Announcement is Published"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Announcement"),
        (status = CONFLICT, description = "The Announcement is already Published"),
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

/// Takes a Published Announcement off the public site, back to Draft; it keeps its slug.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// Announcement, `409` when it is a Draft.
#[utoipa::path(
    post,
    path = "/announcements/{id}/unpublish",
    operation_id = "unpublishAnnouncement",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Announcement")),
    responses(
        (status = NO_CONTENT, description = "The Announcement is a Draft again"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Announcement"),
        (status = CONFLICT, description = "The Announcement is a Draft"),
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
        "SELECT published FROM announcements WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if was == published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    saved(
        sqlx::query!(
            "UPDATE announcements SET published = $1,
             first_published_at = coalesce(first_published_at, now()) WHERE id = $2",
            published,
            id,
        )
        .execute(&mut *tx)
        .await,
    )?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        if published {
            "announcement.publish"
        } else {
            "announcement.unpublish"
        },
        ("announcement", Some(&id.to_string())),
        json!([{ "field": "published", "from": was, "to": published }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a Draft for good; its slug is free again. A Published one is unpublished first.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown
/// Announcement, `409` when it is Published.
#[utoipa::path(
    delete,
    path = "/announcements/{id}",
    operation_id = "deleteAnnouncement",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Announcement")),
    responses(
        (status = NO_CONTENT, description = "The Draft is deleted"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Announcement"),
        (status = CONFLICT, description = "The Announcement is Published"),
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
        "SELECT published FROM announcements WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    let title = sqlx::query_scalar!(
        "DELETE FROM announcements WHERE id = $1 RETURNING title",
        id
    )
    .fetch_one(&mut *tx)
    .await?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "announcement.delete",
        ("announcement", Some(&id.to_string())),
        json!([{ "field": "title", "from": title, "to": null }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// How many Published Announcements a page of the public list holds.
const PAGE_SIZE: i64 = 10;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AnnouncementFilter {
    /// From 1, the default.
    page: Option<u32>,
}

/// A Published Announcement in a public list.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementCard {
    slug: String,
    title: String,
    summary: String,
    start_date: NaiveDate,
    end_date: NaiveDate,
    /// Today's, in the Village's time zone.
    validity: Validity,
    /// Whether it has a detail page.
    has_body: bool,
}

/// One page of the public Announcement list.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementPage {
    announcements: Vec<AnnouncementCard>,
    /// How many Published Announcements there are, over every page.
    total: i64,
    page_size: i64,
}

/// Published Announcements whose period overlaps `[from, to]`, the latest start first.
async fn cards(
    pool: &PgPool,
    today: NaiveDate,
    period: (Option<NaiveDate>, Option<NaiveDate>),
    limit: Option<i64>,
    offset: i64,
) -> sqlx::Result<Vec<AnnouncementCard>> {
    let rows = sqlx::query!(
        r#"SELECT slug, title, summary AS "summary!", start_date AS "start_date!",
           end_date AS "end_date!", has_body AS "has_body!"
           FROM announcements
           WHERE published AND ($1::date IS NULL OR end_date >= $1)
           AND ($2::date IS NULL OR start_date <= $2)
           ORDER BY start_date DESC, id DESC LIMIT $3 OFFSET $4"#,
        period.0,
        period.1,
        limit,
        offset,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| AnnouncementCard {
            validity: Validity::on(today, row.start_date, row.end_date),
            slug: row.slug,
            title: row.title,
            summary: row.summary,
            start_date: row.start_date,
            end_date: row.end_date,
            has_body: row.has_body,
        })
        .collect())
}

/// Every Published Announcement, ended ones included, the latest start first, a page at a time.
#[utoipa::path(
    get,
    path = "/announcements/published",
    operation_id = "listPublishedAnnouncements",
    tag = "content",
    params(AnnouncementFilter),
    responses((status = OK, description = "A page of Published Announcements", body = AnnouncementPage)),
)]
pub(crate) async fn list_published(
    State(pool): State<PgPool>,
    State(clock): State<Clock>,
    Query(filter): Query<AnnouncementFilter>,
) -> Result<Json<AnnouncementPage>, ApiError> {
    let today = village::today(&pool, clock.now()).await?;
    let offset = (i64::from(filter.page.unwrap_or(1).max(1)) - 1) * PAGE_SIZE;
    let total =
        sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM announcements WHERE published"#)
            .fetch_one(&pool)
            .await?;
    let announcements = cards(&pool, today, (None, None), Some(PAGE_SIZE), offset).await?;
    Ok(Json(AnnouncementPage {
        announcements,
        total,
        page_size: PAGE_SIZE,
    }))
}

/// The Published Announcements in effect today, the latest start first, for the home page.
#[utoipa::path(
    get,
    path = "/announcements/in-effect",
    operation_id = "listAnnouncementsInEffect",
    tag = "content",
    responses((status = OK, description = "The Announcements in effect", body = Vec<AnnouncementCard>)),
)]
pub(crate) async fn list_in_effect(
    State(pool): State<PgPool>,
    State(clock): State<Clock>,
) -> Result<Json<Vec<AnnouncementCard>>, ApiError> {
    let today = village::today(&pool, clock.now()).await?;
    Ok(Json(
        cards(&pool, today, (Some(today), Some(today)), None, 0).await?,
    ))
}

/// A Published Announcement with a body, as the public reads it.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishedAnnouncement {
    slug: String,
    title: String,
    summary: String,
    start_date: NaiveDate,
    end_date: NaiveDate,
    validity: Validity,
    /// Tiptap JSON of the allowed nodes and marks only.
    #[schema(value_type = Object)]
    body: Value,
    /// The first publication.
    published_at: DateTime<Utc>,
}

/// One Published Announcement by its slug; only one with a body has a detail page.
///
/// # Errors
/// `404` for a slug no Published Announcement with a body has.
#[utoipa::path(
    get,
    path = "/announcements/published/{slug}",
    operation_id = "getPublishedAnnouncement",
    tag = "content",
    params(("slug" = String, Path, description = "The Announcement's slug")),
    responses(
        (status = OK, description = "The Announcement", body = PublishedAnnouncement),
        (status = NOT_FOUND, description = "No Published Announcement with a body has the slug"),
    ),
)]
pub(crate) async fn get_published(
    State(pool): State<PgPool>,
    State(clock): State<Clock>,
    Path(slug): Path<String>,
) -> Result<Json<PublishedAnnouncement>, ApiError> {
    let today = village::today(&pool, clock.now()).await?;
    let row = sqlx::query!(
        r#"SELECT slug, title, summary AS "summary!", start_date AS "start_date!",
           end_date AS "end_date!", body, first_published_at AS "published_at!"
           FROM announcements WHERE published AND has_body AND slug = $1"#,
        slug,
    )
    .fetch_optional(&pool)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    Ok(Json(PublishedAnnouncement {
        validity: Validity::on(today, row.start_date, row.end_date),
        slug: row.slug,
        title: row.title,
        summary: row.summary,
        start_date: row.start_date,
        end_date: row.end_date,
        body: row.body,
        published_at: row.published_at,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_dates_are_inclusive() {
        let day = |d| NaiveDate::from_ymd_opt(2026, 11, d).unwrap();
        assert_eq!(Validity::on(day(9), day(10), day(12)), Validity::Upcoming);
        assert_eq!(Validity::on(day(10), day(10), day(12)), Validity::InEffect);
        assert_eq!(Validity::on(day(12), day(10), day(12)), Validity::InEffect);
        assert_eq!(Validity::on(day(13), day(10), day(12)), Validity::Ended);
    }
}
