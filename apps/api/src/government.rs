//! The Village government: Positions, the Officials who hold them, and their Tenures.

use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    ApiError,
    audit::{self, Actor},
    auth::Session,
    imaging::Kind,
    role::Permission,
    storage::{self, Storage},
    village::{TimeZone, Upload, diff},
};

/// How an Official holds a Position.
#[derive(Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "tenure_kind", rename_all = "lowercase")]
pub enum TenureKind {
    /// Definitif.
    Definitive,
    /// Pj., a temporary appointee.
    Appointee,
    /// Plt., an acting holder, who may overlap another Tenure.
    Acting,
}

/// What a User records about a Tenure; the name and NIPD are its Official's.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TenureFields {
    /// The Official's name.
    name: String,
    /// The Official's Nomor Induk Perangkat Desa, never shown publicly.
    #[serde(default)]
    nipd: Option<String>,
    kind: TenureKind,
    start_date: NaiveDate,
    /// Open while the Official still serves.
    #[serde(default)]
    end_date: Option<NaiveDate>,
    /// The appointing decree, never shown publicly.
    decree_number: String,
    decree_date: NaiveDate,
    /// The photo's file, served at `/media/{photo}/{variant}`.
    #[serde(default)]
    photo: Option<Uuid>,
    /// Required with a photo.
    #[serde(default)]
    photo_alt: Option<String>,
}

impl TenureFields {
    /// Trims every text, blank to null, and refuses with `422` what is malformed.
    fn normalized(self) -> Result<Self, ApiError> {
        let text = |value: Option<String>, max: usize| match value.as_deref().map(str::trim) {
            None | Some("") => Ok(None),
            Some(value) if value.chars().count() <= max => Ok(Some(value.to_owned())),
            Some(_) => Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY)),
        };
        let required = |value: String, max: usize| {
            text(Some(value), max)?.ok_or(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))
        };
        let fields = Self {
            name: required(self.name, 100)?,
            nipd: text(self.nipd, 30)?,
            decree_number: required(self.decree_number, 100)?,
            photo_alt: text(self.photo_alt, 200)?,
            ..self
        };
        if fields.end_date.is_none_or(|end| end >= fields.start_date)
            && fields.photo.is_some() == fields.photo_alt.is_some()
        {
            Ok(fields)
        } else {
            Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Tenure {
    id: Uuid,
    official_id: Uuid,
    #[serde(flatten)]
    fields: TenureFields,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    id: Uuid,
    /// A Head-of-Hamlet Position is named after its Hamlet, such as "Kepala Dusun Krajan".
    name: String,
    /// The head of village's Position, which is never deleted.
    head_of_village: bool,
    /// A Head-of-Hamlet Position's Hamlet, which it is created and deactivated with.
    hamlet_id: Option<Uuid>,
    /// An inactive Position takes no new Tenure.
    active: bool,
    /// Latest start first.
    tenures: Vec<Tenure>,
}

#[derive(Deserialize, ToSchema)]
pub struct PositionName {
    /// 1–100 characters.
    name: String,
}

impl PositionName {
    fn normalized(self) -> Result<String, ApiError> {
        let name = self.name.trim();
        if (1..=100).contains(&name.chars().count()) {
            Ok(name.to_owned())
        } else {
            Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))
        }
    }
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewTenure {
    position_id: Uuid,
    /// An existing Official, whose name and NIPD the body's replace; null for a new one.
    #[serde(default)]
    official_id: Option<Uuid>,
    #[serde(flatten)]
    fields: TenureFields,
}

#[derive(Serialize, ToSchema)]
pub struct Created {
    id: Uuid,
}

fn ensure_manager(session: &Session) -> Result<(), ApiError> {
    if session.holds(Permission::VillageManage) {
        Ok(())
    } else {
        Err(ApiError::Status(StatusCode::FORBIDDEN))
    }
}

/// A foreign key, such as an unknown photo or Official, says the body is wrong.
fn known<T>(result: sqlx::Result<T>, status: StatusCode) -> Result<T, ApiError> {
    match result {
        Err(sqlx::Error::Database(error)) if error.is_foreign_key_violation() => {
            Err(ApiError::Status(status))
        }
        result => Ok(result?),
    }
}

/// Locks a Position so its Tenures are checked one change at a time; whether it is active.
async fn lock_position(conn: &mut sqlx::PgConnection, id: Uuid) -> Result<bool, ApiError> {
    sqlx::query_scalar!("SELECT active FROM positions WHERE id = $1 FOR UPDATE", id)
        .fetch_optional(conn)
        .await?
        .ok_or(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))
}

/// Whether a Definitif or Pj. Tenure of the Position, other than `except`, overlaps the days.
async fn overlaps(
    conn: &mut sqlx::PgConnection,
    position_id: Uuid,
    except: Option<Uuid>,
    fields: &TenureFields,
) -> sqlx::Result<bool> {
    if fields.kind == TenureKind::Acting {
        return Ok(false);
    }
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM tenures WHERE position_id = $1 AND kind <> 'acting'
           AND id IS DISTINCT FROM $2
           AND daterange(start_date, end_date, '[]') && daterange($3, $4, '[]')) AS "exists!""#,
        position_id,
        except,
        fields.start_date,
        fields.end_date,
    )
    .fetch_one(conn)
    .await
}

/// Every Position in the government's order with its Tenures, for any signed-in User.
///
/// # Errors
/// `401` without a live session.
#[utoipa::path(
    get,
    path = "/positions",
    operation_id = "listPositions",
    tag = "government",
    responses(
        (status = OK, description = "Every Position", body = Vec<Position>),
        (status = UNAUTHORIZED, description = "There is no live session"),
    ),
)]
pub(crate) async fn list_positions(
    State(pool): State<PgPool>,
    _session: Session,
) -> Result<Json<Vec<Position>>, ApiError> {
    let mut tenures = sqlx::query!(
        r#"SELECT tenures.id, position_id, official_id, name, nipd,
           kind AS "kind: TenureKind", start_date, end_date, decree_number, decree_date,
           photo_file_id, photo_alt
           FROM tenures JOIN officials ON officials.id = official_id
           ORDER BY start_date DESC, tenures.id DESC"#
    )
    .fetch_all(&pool)
    .await?;
    let positions = sqlx::query!(
        r#"SELECT positions.id, coalesce(positions.name,
               'Kepala ' || initcap(village.hamlet_label) || ' ' || hamlets.name) AS "name!",
           head_of_village, hamlet_id, positions.active
           FROM positions LEFT JOIN region_units hamlets ON hamlets.id = hamlet_id
           LEFT JOIN village ON true
           ORDER BY hamlet_id IS NOT NULL, positions.sort_order, hamlets.sort_order"#
    )
    .fetch_all(&pool)
    .await?
    .into_iter()
    .map(|row| Position {
        tenures: tenures
            .extract_if(.., |tenure| tenure.position_id == row.id)
            .map(|tenure| Tenure {
                id: tenure.id,
                official_id: tenure.official_id,
                fields: TenureFields {
                    name: tenure.name,
                    nipd: tenure.nipd,
                    kind: tenure.kind,
                    start_date: tenure.start_date,
                    end_date: tenure.end_date,
                    decree_number: tenure.decree_number,
                    decree_date: tenure.decree_date,
                    photo: tenure.photo_file_id,
                    photo_alt: tenure.photo_alt,
                },
            })
            .collect(),
        id: row.id,
        name: row.name,
        head_of_village: row.head_of_village,
        hamlet_id: row.hamlet_id,
        active: row.active,
    })
    .collect();
    Ok(Json(positions))
}

/// Adds a Village-defined Position at the end of the government's order.
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `422` for a blank name or
/// one over 100 characters.
#[utoipa::path(
    post,
    path = "/positions",
    operation_id = "createPosition",
    tag = "government",
    request_body = PositionName,
    responses(
        (status = CREATED, description = "The new Position", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = UNPROCESSABLE_ENTITY, description = "The name is blank or too long"),
    ),
)]
pub(crate) async fn create_position(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<PositionName>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure_manager(&session)?;
    let name = body.normalized()?;
    let mut tx = pool.begin().await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO positions (name, sort_order)
         SELECT $1, coalesce(max(sort_order), 0) + 1 FROM positions RETURNING id",
        name,
    )
    .fetch_one(&mut *tx)
    .await?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "position.create",
        ("position", Some(&id.to_string())),
        json!([{ "field": "name", "from": null, "to": name }]),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

/// Renames a Position other than a Head-of-Hamlet one, which takes its Hamlet's name.
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `404` for an unknown
/// Position, `409` for a Head-of-Hamlet Position, `422` for a blank name or one over 100
/// characters.
#[utoipa::path(
    put,
    path = "/positions/{id}",
    operation_id = "renamePosition",
    tag = "government",
    params(("id" = Uuid, Path, description = "The Position")),
    request_body = PositionName,
    responses(
        (status = NO_CONTENT, description = "The Position is renamed"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = NOT_FOUND, description = "There is no such Position"),
        (status = CONFLICT, description = "A Head-of-Hamlet Position is named after its Hamlet"),
        (status = UNPROCESSABLE_ENTITY, description = "The name is blank or too long"),
    ),
)]
pub(crate) async fn rename_position(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<PositionName>,
) -> Result<StatusCode, ApiError> {
    ensure_manager(&session)?;
    let name = body.normalized()?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query_scalar!("SELECT name FROM positions WHERE id = $1 FOR UPDATE", id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?
        .ok_or(ApiError::Status(StatusCode::CONFLICT))?;
    if old != name {
        sqlx::query!("UPDATE positions SET name = $1 WHERE id = $2", name, id)
            .execute(&mut *tx)
            .await?;
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "position.update",
            ("position", Some(&id.to_string())),
            json!([{ "field": "name", "from": old, "to": name }]),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes a Position that never had a Tenure, other than the head of village's and the
/// Head-of-Hamlet ones.
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `404` for an unknown
/// Position, `409` for the head of village's, a Head-of-Hamlet one, or one with a Tenure.
#[utoipa::path(
    delete,
    path = "/positions/{id}",
    operation_id = "deletePosition",
    tag = "government",
    params(("id" = Uuid, Path, description = "The Position")),
    responses(
        (status = NO_CONTENT, description = "The Position is deleted"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = NOT_FOUND, description = "There is no such Position"),
        (status = CONFLICT, description = "The Position is kept"),
    ),
)]
pub(crate) async fn delete_position(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    ensure_manager(&session)?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        "SELECT name, head_of_village FROM positions WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    let (Some(name), false) = (old.name, old.head_of_village) else {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    };
    known(
        sqlx::query!("DELETE FROM positions WHERE id = $1", id)
            .execute(&mut *tx)
            .await,
        StatusCode::CONFLICT,
    )?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "position.delete",
        ("position", Some(&id.to_string())),
        json!([{ "field": "name", "from": name, "to": null }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Records a Tenure of a new or existing Official in an active Position.
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `409` for an inactive
/// Position or a Definitif or Pj. Tenure overlapping another, `422` for a malformed field, an
/// end before the start, a photo without alt text, or an unknown Position, Official or photo.
#[utoipa::path(
    post,
    path = "/tenures",
    operation_id = "createTenure",
    tag = "government",
    request_body = NewTenure,
    responses(
        (status = CREATED, description = "The new Tenure", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = CONFLICT, description = "The Position is inactive or already held then"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed or unknown"),
    ),
)]
pub(crate) async fn create_tenure(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<NewTenure>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure_manager(&session)?;
    let fields = body.fields.normalized()?;
    let mut tx = pool.begin().await?;
    if !lock_position(&mut tx, body.position_id).await?
        || overlaps(&mut tx, body.position_id, None, &fields).await?
    {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    let official_id = match body.official_id {
        Some(id) => sqlx::query_scalar!(
            "UPDATE officials SET name = $1, nipd = $2 WHERE id = $3 RETURNING id",
            fields.name,
            fields.nipd,
            id,
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))?,
        None => {
            sqlx::query_scalar!(
                "INSERT INTO officials (name, nipd) VALUES ($1, $2) RETURNING id",
                fields.name,
                fields.nipd,
            )
            .fetch_one(&mut *tx)
            .await?
        }
    };
    let id = known(
        sqlx::query_scalar!(
            "INSERT INTO tenures (position_id, official_id, kind, start_date, end_date,
             decree_number, decree_date, photo_file_id, photo_alt)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING id",
            body.position_id,
            official_id,
            fields.kind as TenureKind,
            fields.start_date,
            fields.end_date,
            fields.decree_number,
            fields.decree_date,
            fields.photo,
            fields.photo_alt,
        )
        .fetch_one(&mut *tx)
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
    )?;
    let mut changes = diff(
        &Value::Null,
        &json!({ "positionId": body.position_id, "officialId": official_id }),
    );
    changes.extend(diff(&Value::Null, &serde_json::to_value(&fields)?));
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "tenure.create",
        ("tenure", Some(&id.to_string())),
        changes.into(),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

/// Corrects or ends a Tenure, and its Official's name and NIPD; a Tenure is never deleted.
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `404` for an unknown
/// Tenure, `409` for a Definitif or Pj. Tenure overlapping another or an open one in an
/// inactive Position, `422` for a malformed field, an end before the start, a photo without
/// alt text, or an unknown photo.
#[utoipa::path(
    put,
    path = "/tenures/{id}",
    operation_id = "updateTenure",
    tag = "government",
    params(("id" = Uuid, Path, description = "The Tenure")),
    request_body = TenureFields,
    responses(
        (status = NO_CONTENT, description = "The Tenure is changed"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = NOT_FOUND, description = "There is no such Tenure"),
        (status = CONFLICT, description = "The Position is already held then, or inactive"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed or unknown"),
    ),
)]
pub(crate) async fn update_tenure(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<TenureFields>,
) -> Result<StatusCode, ApiError> {
    ensure_manager(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        r#"SELECT position_id, official_id, name, nipd, kind AS "kind: TenureKind", start_date,
           end_date, decree_number, decree_date, photo_file_id, photo_alt
           FROM tenures JOIN officials ON officials.id = official_id
           WHERE tenures.id = $1 FOR UPDATE OF tenures"#,
        id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    let active = lock_position(&mut tx, old.position_id).await?;
    if !active && new.end_date.is_none()
        || overlaps(&mut tx, old.position_id, Some(id), &new).await?
    {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    sqlx::query!(
        "UPDATE officials SET name = $1, nipd = $2 WHERE id = $3",
        new.name,
        new.nipd,
        old.official_id,
    )
    .execute(&mut *tx)
    .await?;
    known(
        sqlx::query!(
            "UPDATE tenures SET kind = $1, start_date = $2, end_date = $3, decree_number = $4,
             decree_date = $5, photo_file_id = $6, photo_alt = $7 WHERE id = $8",
            new.kind as TenureKind,
            new.start_date,
            new.end_date,
            new.decree_number,
            new.decree_date,
            new.photo,
            new.photo_alt,
            id,
        )
        .execute(&mut *tx)
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
    )?;
    if let Some(photo) = old.photo_file_id.filter(|&photo| Some(photo) != new.photo) {
        storage::release(&mut tx, photo).await?;
    }
    let from = serde_json::to_value(TenureFields {
        name: old.name,
        nipd: old.nipd,
        kind: old.kind,
        start_date: old.start_date,
        end_date: old.end_date,
        decree_number: old.decree_number,
        decree_date: old.decree_date,
        photo: old.photo_file_id,
        photo_alt: old.photo_alt,
    })?;
    let changes = diff(&from, &serde_json::to_value(&new)?);
    if !changes.is_empty() {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "tenure.update",
            ("tenure", Some(&id.to_string())),
            changes.into(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Stores an Official's photo, a JPEG, PNG or WebP of at most 10 MiB, for saving into a
/// Tenure.
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `413` over 10 MiB, `415`
/// for content that is not JPEG, PNG or WebP, `422` over 40 megapixels or undecodable.
#[utoipa::path(
    post,
    path = "/tenures/photo",
    operation_id = "uploadTenurePhoto",
    tag = "government",
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = CREATED, description = "The stored photo", body = Upload),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = PAYLOAD_TOO_LARGE, description = "The file is over 10 MiB"),
        (status = UNSUPPORTED_MEDIA_TYPE, description = "The file is not JPEG, PNG or WebP"),
        (status = UNPROCESSABLE_ENTITY, description = "The image is too large or broken"),
    ),
)]
pub(crate) async fn upload_photo(
    State(pool): State<PgPool>,
    State(storage): State<Arc<Storage>>,
    session: Session,
    body: Body,
) -> Result<(StatusCode, Json<Upload>), ApiError> {
    ensure_manager(&session)?;
    let upload = axum::body::to_bytes(body, Kind::Photo.max_bytes())
        .await
        .map_err(|_| ApiError::Status(StatusCode::PAYLOAD_TOO_LARGE))?;
    let id = storage
        .save_image(&pool, Kind::Photo, upload.into())
        .await?;
    Ok((StatusCode::CREATED, Json(Upload { id })))
}

/// An Official serving today, as the public sees them.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServingOfficial {
    name: String,
    position: String,
    kind: TenureKind,
    photo: Option<Uuid>,
    photo_alt: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct PublicHamlet {
    id: Uuid,
    name: String,
    /// The names of the Officials serving today as its head.
    heads: Vec<String>,
}

/// The government structure on the public site; never a NIPD or decree.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Government {
    /// Serving today in the Village's time zone, in the government's order.
    officials: Vec<ServingOfficial>,
    /// The local name for a Hamlet, such as dusun.
    hamlet_label: String,
    /// The active Hamlets in their manual order.
    hamlets: Vec<PublicHamlet>,
}

/// The Officials serving today and the Hamlets with their heads, for the public site.
///
/// # Errors
/// `404` before the Village Code is set.
#[utoipa::path(
    get,
    path = "/government",
    operation_id = "getGovernment",
    tag = "government",
    responses(
        (status = OK, description = "The government structure", body = Government),
        (status = NOT_FOUND, description = "The Village Code is not set yet"),
    ),
)]
pub(crate) async fn get_government(
    State(pool): State<PgPool>,
) -> Result<Json<Government>, ApiError> {
    let village = sqlx::query!("SELECT code, hamlet_label FROM village")
        .fetch_optional(&pool)
        .await?
        .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    let serving = sqlx::query!(
        r#"SELECT officials.name, coalesce(positions.name,
               'Kepala ' || initcap($2) || ' ' || hamlets.name) AS "position!",
           tenures.kind AS "kind: TenureKind", photo_file_id, photo_alt, hamlet_id
           FROM tenures JOIN officials ON officials.id = official_id
           JOIN positions ON positions.id = position_id
           LEFT JOIN region_units hamlets ON hamlets.id = hamlet_id
           CROSS JOIN LATERAL (SELECT (now() AT TIME ZONE $1)::date AS today) AS now
           WHERE positions.active AND start_date <= today AND (end_date IS NULL OR end_date >= today)
           ORDER BY hamlet_id IS NOT NULL, positions.sort_order, hamlets.sort_order, tenures.kind,
               start_date"#,
        TimeZone::of_province(&village.code).iana(),
        village.hamlet_label,
    )
    .fetch_all(&pool)
    .await?;
    let hamlets = sqlx::query!(
        "SELECT id, name AS \"name!\" FROM region_units WHERE kind = 'hamlet' AND active
         ORDER BY sort_order"
    )
    .fetch_all(&pool)
    .await?
    .into_iter()
    .map(|hamlet| PublicHamlet {
        heads: serving
            .iter()
            .filter(|official| official.hamlet_id == Some(hamlet.id))
            .map(|official| official.name.clone())
            .collect(),
        id: hamlet.id,
        name: hamlet.name,
    })
    .collect();
    let officials = serving
        .into_iter()
        .map(|official| ServingOfficial {
            name: official.name,
            position: official.position,
            kind: official.kind,
            photo: official.photo_file_id,
            photo_alt: official.photo_alt,
        })
        .collect();
    Ok(Json(Government {
        officials,
        hamlet_label: village.hamlet_label,
        hamlets,
    }))
}
