//! Events: scheduled village activities on the Agenda, written as Drafts and then Published.
//! A Cancelled Event stays listed; whether one is upcoming or past is computed from the
//! Village's wall clock by the database's `event_is_past`.
//!
//! `content.write` creates and edits Drafts; `content.publish` also publishes, unpublishes,
//! edits Published Events (cancelling one included) and deletes Drafts.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::{DateTime, NaiveDateTime, Utc};
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
    village::{self, diff},
};

/// What a User writes in an Event. A Draft may leave out what publishing requires.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EventFields {
    /// 1–150 characters.
    title: String,
    /// The Category it is filed under, from the list News Posts share; required to publish.
    #[serde(default)]
    category: Option<Uuid>,
    /// Wall-clock time in the Village's time zone; required to publish.
    #[serde(default)]
    starts_at: Option<NaiveDateTime>,
    /// Optional, not before the start, possibly days later. Without one, the Event is past
    /// once it has started.
    #[serde(default)]
    ends_at: Option<NaiveDateTime>,
    /// Plain text, at most 200 characters; required to publish.
    #[serde(default)]
    location: Option<String>,
    /// Optional plain text, at most 2000 characters.
    #[serde(default)]
    description: Option<String>,
    /// Called off; it stays listed, labelled "Dibatalkan".
    #[serde(default)]
    cancelled: bool,
}

impl EventFields {
    /// Trims every text, blank to null, and refuses with `422` what is malformed.
    fn normalized(self) -> Result<Self, ApiError> {
        let text = |value: Option<String>, max: usize| match value.as_deref().map(str::trim) {
            None | Some("") => Ok(None),
            Some(value) if value.chars().count() <= max => Ok(Some(value.to_owned())),
            Some(_) => Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY)),
        };
        if let (Some(start), Some(end)) = (self.starts_at, self.ends_at)
            && end < start
        {
            return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
        }
        Ok(Self {
            title: text(Some(self.title), 150)?
                .ok_or(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))?,
            category: self.category,
            starts_at: self.starts_at,
            ends_at: self.ends_at,
            location: text(self.location, 200)?,
            description: text(self.description, 2000)?,
            cancelled: self.cancelled,
        })
    }

    /// The fields as an Audit Entry shows them.
    fn audited(&self) -> Value {
        json!({
            "title": self.title, "category": self.category, "startsAt": self.starts_at,
            "endsAt": self.ends_at, "location": self.location, "description": self.description,
            "cancelled": self.cancelled,
        })
    }
}

/// Whether an Event is still to come (or under way) or over.
#[derive(Serialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Timing {
    Upcoming,
    Past,
}

/// An Event as the admin area edits it.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    id: Uuid,
    #[serde(flatten)]
    fields: EventFields,
    published: bool,
    first_published_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

/// A line of the admin area's Event list.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventSummary {
    id: Uuid,
    title: String,
    /// The Category's name.
    category: Option<String>,
    starts_at: Option<NaiveDateTime>,
    ends_at: Option<NaiveDateTime>,
    /// Now's, once the start is set.
    timing: Option<Timing>,
    cancelled: bool,
    published: bool,
    updated_at: DateTime<Utc>,
    /// The creating User's username.
    author: String,
}

/// An unknown author or Category says the body is wrong, and so does a Published Event
/// missing what publishing requires.
fn saved<T>(result: sqlx::Result<T>) -> Result<T, ApiError> {
    match result {
        Err(sqlx::Error::Database(error))
            if error.is_foreign_key_violation() || error.is_check_violation() =>
        {
            Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))
        }
        result => Ok(result?),
    }
}

/// Every Event, Drafts and Published, the latest changed first.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`.
#[utoipa::path(
    get,
    path = "/events",
    operation_id = "listEvents",
    tag = "content",
    responses(
        (status = OK, description = "Every Event", body = Vec<EventSummary>),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
    ),
)]
pub(crate) async fn list(
    State(pool): State<PgPool>,
    State(clock): State<Clock>,
    session: Session,
) -> Result<Json<Vec<EventSummary>>, ApiError> {
    ensure_writer(&session)?;
    let now = village::wall_clock(&pool, clock.now()).await?;
    let rows = sqlx::query!(
        "SELECT events.id, title, categories.name AS \"category?\", starts_at, ends_at,
         event_is_past(starts_at, ends_at, $1) AS past, cancelled, published, updated_at,
         users.username AS author
         FROM events JOIN users ON users.id = author_id
         LEFT JOIN categories ON categories.id = category_id
         ORDER BY updated_at DESC, events.id DESC",
        now,
    )
    .fetch_all(&pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| EventSummary {
                id: row.id,
                title: row.title,
                category: row.category,
                starts_at: row.starts_at,
                ends_at: row.ends_at,
                timing: row
                    .past
                    .map(|past| if past { Timing::Past } else { Timing::Upcoming }),
                cancelled: row.cancelled,
                published: row.published,
                updated_at: row.updated_at,
                author: row.author,
            })
            .collect(),
    ))
}

/// One Event, Draft or Published, for editing.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `404`
/// for an unknown Event.
#[utoipa::path(
    get,
    path = "/events/{id}",
    operation_id = "getEvent",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Event")),
    responses(
        (status = OK, description = "The Event", body = Event),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
        (status = NOT_FOUND, description = "There is no such Event"),
    ),
)]
pub(crate) async fn get(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<Json<Event>, ApiError> {
    ensure_writer(&session)?;
    let row = sqlx::query!(
        "SELECT title, category_id, starts_at, ends_at, location, description, cancelled,
         published, first_published_at, updated_at FROM events WHERE id = $1",
        id
    )
    .fetch_optional(&pool)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    Ok(Json(Event {
        id,
        fields: EventFields {
            title: row.title,
            category: row.category_id,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
            location: row.location,
            description: row.description,
            cancelled: row.cancelled,
        },
        published: row.published,
        first_published_at: row.first_published_at,
        updated_at: row.updated_at,
    }))
}

/// Starts a Draft written by the signed-in User; only the title is required.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` or `content.publish`, `422`
/// for a blank title, a malformed field, an end before the start or an unknown Category.
#[utoipa::path(
    post,
    path = "/events",
    operation_id = "createEvent",
    tag = "content",
    request_body = EventFields,
    responses(
        (status = CREATED, description = "The new Draft", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not write content"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed"),
    ),
)]
pub(crate) async fn create(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<EventFields>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure_writer(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let id = saved(
        sqlx::query_scalar!(
            "INSERT INTO events (title, category_id, starts_at, ends_at, location, description,
             cancelled, author_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
            new.title,
            new.category,
            new.starts_at,
            new.ends_at,
            new.location,
            new.description,
            new.cancelled,
            session.user_id,
        )
        .fetch_one(&mut *tx)
        .await,
    )?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "event.create",
        ("event", Some(&id.to_string())),
        diff(&Value::Null, &new.audited()).into(),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

/// Saves an Event: a Draft as it autosaves, a Published one live.
///
/// # Errors
/// `401` without a live session, `403` without `content.write` (or `content.publish`) for a
/// Draft or without `content.publish` for a Published Event, `404` for an unknown Event, `422`
/// for a blank title, a malformed field, an end before the start, an unknown Category, or a
/// Published Event missing what publishing requires.
#[utoipa::path(
    put,
    path = "/events/{id}",
    operation_id = "updateEvent",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Event")),
    request_body = EventFields,
    responses(
        (status = NO_CONTENT, description = "The Event is saved"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not change this Event"),
        (status = NOT_FOUND, description = "There is no such Event"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed or missing"),
    ),
)]
pub(crate) async fn update(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<EventFields>,
) -> Result<StatusCode, ApiError> {
    ensure_writer(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        "SELECT title, category_id, starts_at, ends_at, location, description, cancelled,
         published FROM events WHERE id = $1 FOR UPDATE",
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
            "UPDATE events SET title = $1, category_id = $2, starts_at = $3, ends_at = $4,
             location = $5, description = $6, cancelled = $7, updated_at = now() WHERE id = $8",
            new.title,
            new.category,
            new.starts_at,
            new.ends_at,
            new.location,
            new.description,
            new.cancelled,
            id,
        )
        .execute(&mut *tx)
        .await,
    )?;
    let from = EventFields {
        title: old.title,
        category: old.category_id,
        starts_at: old.starts_at,
        ends_at: old.ends_at,
        location: old.location,
        description: old.description,
        cancelled: old.cancelled,
    }
    .audited();
    let changes = diff(&from, &new.audited());
    if !changes.is_empty() {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "event.update",
            ("event", Some(&id.to_string())),
            changes.into(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Puts a Draft on the public Agenda.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown Event,
/// `409` when it is already Published, `422` when it lacks a Category, start or location.
#[utoipa::path(
    post,
    path = "/events/{id}/publish",
    operation_id = "publishEvent",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Event")),
    responses(
        (status = NO_CONTENT, description = "The Event is Published"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Event"),
        (status = CONFLICT, description = "The Event is already Published"),
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

/// Takes a Published Event off the public Agenda, back to Draft.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown Event,
/// `409` when it is a Draft.
#[utoipa::path(
    post,
    path = "/events/{id}/unpublish",
    operation_id = "unpublishEvent",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Event")),
    responses(
        (status = NO_CONTENT, description = "The Event is a Draft again"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Event"),
        (status = CONFLICT, description = "The Event is a Draft"),
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
    let was = sqlx::query_scalar!("SELECT published FROM events WHERE id = $1 FOR UPDATE", id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if was == published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    saved(
        sqlx::query!(
            "UPDATE events SET published = $1,
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
            "event.publish"
        } else {
            "event.unpublish"
        },
        ("event", Some(&id.to_string())),
        json!([{ "field": "published", "from": was, "to": published }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a Draft for good. A Published one is unpublished first.
///
/// # Errors
/// `401` without a live session, `403` without `content.publish`, `404` for an unknown Event,
/// `409` when it is Published.
#[utoipa::path(
    delete,
    path = "/events/{id}",
    operation_id = "deleteEvent",
    tag = "content",
    params(("id" = Uuid, Path, description = "The Event")),
    responses(
        (status = NO_CONTENT, description = "The Draft is deleted"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not publish content"),
        (status = NOT_FOUND, description = "There is no such Event"),
        (status = CONFLICT, description = "The Event is Published"),
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
        sqlx::query_scalar!("SELECT published FROM events WHERE id = $1 FOR UPDATE", id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if published {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    let title = sqlx::query_scalar!("DELETE FROM events WHERE id = $1 RETURNING title", id)
        .fetch_one(&mut *tx)
        .await?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "event.delete",
        ("event", Some(&id.to_string())),
        json!([{ "field": "title", "from": title, "to": null }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// How many past Events a page of the public Agenda holds.
const PAGE_SIZE: i64 = 10;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct EventFilter {
    /// From 1, the default.
    page: Option<u32>,
}

/// A Published Event on the public Agenda; it has no detail page.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventCard {
    id: Uuid,
    title: String,
    /// The Category's name.
    category: String,
    /// Wall-clock time in the Village's time zone.
    starts_at: NaiveDateTime,
    ends_at: Option<NaiveDateTime>,
    location: String,
    description: Option<String>,
    /// Shown as "Dibatalkan".
    cancelled: bool,
}

/// One page of past Events.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventPage {
    events: Vec<EventCard>,
    /// How many past Published Events there are, over every page.
    total: i64,
    page_size: i64,
}

/// Published Events past (latest start first) or not (soonest start first) at `now`.
async fn cards(
    pool: &PgPool,
    now: NaiveDateTime,
    past: bool,
    limit: Option<i64>,
    offset: i64,
) -> sqlx::Result<Vec<EventCard>> {
    sqlx::query_as!(
        EventCard,
        r#"SELECT events.id, title, categories.name AS category, starts_at AS "starts_at!",
           ends_at, location AS "location!", description, cancelled
           FROM events JOIN categories ON categories.id = category_id
           WHERE published AND event_is_past(starts_at, ends_at, $1) = $2
           ORDER BY CASE WHEN $2 THEN starts_at END DESC, starts_at, events.id
           LIMIT $3 OFFSET $4"#,
        now,
        past,
        limit,
        offset,
    )
    .fetch_all(pool)
    .await
}

/// The Published Events still to come or under way, the soonest first, Cancelled ones
/// included.
#[utoipa::path(
    get,
    path = "/events/upcoming",
    operation_id = "listUpcomingEvents",
    tag = "content",
    responses((status = OK, description = "The upcoming Events", body = Vec<EventCard>)),
)]
pub(crate) async fn list_upcoming(
    State(pool): State<PgPool>,
    State(clock): State<Clock>,
) -> Result<Json<Vec<EventCard>>, ApiError> {
    let now = village::wall_clock(&pool, clock.now()).await?;
    Ok(Json(cards(&pool, now, false, None, 0).await?))
}

/// The Published Events that are over, the latest first, a page at a time.
#[utoipa::path(
    get,
    path = "/events/past",
    operation_id = "listPastEvents",
    tag = "content",
    params(EventFilter),
    responses((status = OK, description = "A page of past Events", body = EventPage)),
)]
pub(crate) async fn list_past(
    State(pool): State<PgPool>,
    State(clock): State<Clock>,
    Query(filter): Query<EventFilter>,
) -> Result<Json<EventPage>, ApiError> {
    let now = village::wall_clock(&pool, clock.now()).await?;
    let offset = (i64::from(filter.page.unwrap_or(1).max(1)) - 1) * PAGE_SIZE;
    let total = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM events
           WHERE published AND event_is_past(starts_at, ends_at, $1)"#,
        now,
    )
    .fetch_one(&pool)
    .await?;
    let events = cards(&pool, now, true, Some(PAGE_SIZE), offset).await?;
    Ok(Json(EventPage {
        events,
        total,
        page_size: PAGE_SIZE,
    }))
}
