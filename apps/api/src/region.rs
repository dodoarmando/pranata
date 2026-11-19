//! The Village's region tree: Hamlets, an optional RW level, and RTs.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    ApiError,
    audit::{self, Actor},
    auth::Session,
    role::Permission,
    village::{diff, phone_ok},
};

/// Which level of the tree a Region Unit is.
#[derive(Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "region_kind", rename_all = "lowercase")]
pub enum RegionKind {
    Hamlet,
    Rw,
    Rt,
}

/// What a User edits on a Region Unit: a Hamlet's name, or an RW's or RT's number and head.
#[derive(Serialize, Deserialize, ToSchema, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RegionUnitFields {
    /// A Hamlet's free-text name; null for an RW or RT.
    #[serde(default)]
    name: Option<String>,
    /// An RW's or RT's number, 1–999; null for a Hamlet.
    #[serde(default)]
    number: Option<i32>,
    /// The RW's or RT's head, never shown publicly.
    #[serde(default)]
    head_name: Option<String>,
    #[serde(default)]
    head_phone: Option<String>,
}

impl RegionUnitFields {
    /// Trims every text, blank to null, and refuses with `422` what does not fit `kind`.
    fn normalized(self, kind: RegionKind) -> Result<Self, ApiError> {
        let text = |value: Option<String>| {
            value
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        let fields = Self {
            name: text(self.name),
            number: self.number,
            head_name: text(self.head_name),
            head_phone: text(self.head_phone),
        };
        let ok = match kind {
            RegionKind::Hamlet => {
                fields
                    .name
                    .as_ref()
                    .is_some_and(|name| name.chars().count() <= 100)
                    && fields.number.is_none()
                    && fields.head_name.is_none()
                    && fields.head_phone.is_none()
            }
            RegionKind::Rw | RegionKind::Rt => {
                fields.name.is_none()
                    && fields
                        .number
                        .is_some_and(|number| (1..=999).contains(&number))
                    && fields
                        .head_name
                        .as_ref()
                        .is_none_or(|name| name.chars().count() <= 100)
                    && fields
                        .head_phone
                        .as_deref()
                        .is_none_or(|phone| phone.len() <= 20 && phone_ok(phone))
            }
        };
        if ok {
            Ok(fields)
        } else {
            Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY))
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegionUnit {
    id: Uuid,
    kind: RegionKind,
    /// An RW's Hamlet, or an RT's RW (its Hamlet when the Village has no RW level).
    parent_id: Option<Uuid>,
    #[serde(flatten)]
    fields: RegionUnitFields,
    /// An inactive unit cannot be picked for new references.
    active: bool,
}

/// How the Village names and shapes its tree.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegionSettings {
    /// The local name for a Hamlet, such as dusun, banjar, jorong or lingkungan; kept lowercase.
    hamlet_label: String,
    /// Whether RTs belong to RWs rather than straight to their Hamlet.
    has_rw: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegionTree {
    #[serde(flatten)]
    settings: RegionSettings,
    /// Hamlets in their manual order, then RWs and RTs by number.
    units: Vec<RegionUnit>,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewRegionUnit {
    kind: RegionKind,
    /// Required for an RW or RT, absent for a Hamlet; must be active.
    #[serde(default)]
    parent_id: Option<Uuid>,
    #[serde(flatten)]
    fields: RegionUnitFields,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegionUnitUpdate {
    #[serde(flatten)]
    fields: RegionUnitFields,
    /// False only once every child is inactive; true only under an active parent, and for an RW
    /// only while the Village has an RW level.
    active: bool,
}

#[derive(Serialize, ToSchema)]
pub struct Created {
    id: Uuid,
}

#[derive(Deserialize, ToSchema)]
pub struct HamletOrder {
    /// Every Hamlet, active or not, in its new order.
    ids: Vec<Uuid>,
}

fn ensure_manager(session: &Session) -> Result<(), ApiError> {
    if session.holds(Permission::VillageManage) {
        Ok(())
    } else {
        Err(ApiError::Status(StatusCode::FORBIDDEN))
    }
}

/// The unique number indexes say the number is taken.
fn taken(result: sqlx::Result<Uuid>) -> Result<Uuid, ApiError> {
    match result {
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(ApiError::Status(StatusCode::CONFLICT))
        }
        result => Ok(result?),
    }
}

/// The Village's region tree with its settings, heads included, for any signed-in User.
///
/// # Errors
/// `401` without a live session, `404` before the Village Code is set.
#[utoipa::path(
    get,
    path = "/regions",
    operation_id = "getRegionTree",
    tag = "regions",
    responses(
        (status = OK, description = "The region tree", body = RegionTree),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = NOT_FOUND, description = "The Village Code is not set yet"),
    ),
)]
pub(crate) async fn get_tree(
    State(pool): State<PgPool>,
    _session: Session,
) -> Result<Json<RegionTree>, ApiError> {
    let settings = sqlx::query_as!(RegionSettings, "SELECT hamlet_label, has_rw FROM village")
        .fetch_optional(&pool)
        .await?
        .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    let units = sqlx::query!(
        r#"SELECT id, kind AS "kind: RegionKind", parent_id, name, number, head_name, head_phone,
           active FROM region_units ORDER BY kind, sort_order, number"#
    )
    .fetch_all(&pool)
    .await?
    .into_iter()
    .map(|row| RegionUnit {
        id: row.id,
        kind: row.kind,
        parent_id: row.parent_id,
        fields: RegionUnitFields {
            name: row.name,
            number: row.number,
            head_name: row.head_name,
            head_phone: row.head_phone,
        },
        active: row.active,
    })
    .collect();
    Ok(Json(RegionTree { settings, units }))
}

/// Renames the Hamlet level and turns the RW level on or off, which only while no RT exists
/// and, to turn it off, every RW is inactive.
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `404` before the Village
/// Code is set, `409` for toggling the RW level once an RT exists or turning it off while an RW
/// is active, `422` for a blank label or
/// one over 30 characters.
#[utoipa::path(
    put,
    path = "/regions/settings",
    operation_id = "updateRegionSettings",
    tag = "regions",
    request_body = RegionSettings,
    responses(
        (status = NO_CONTENT, description = "The settings are saved"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = NOT_FOUND, description = "The Village Code is not set yet"),
        (status = CONFLICT, description = "The RW level changes while an RT or active RW exists"),
        (status = UNPROCESSABLE_ENTITY, description = "The label is blank or too long"),
    ),
)]
pub(crate) async fn update_settings(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<RegionSettings>,
) -> Result<StatusCode, ApiError> {
    ensure_manager(&session)?;
    let new = RegionSettings {
        hamlet_label: body.hamlet_label.trim().to_lowercase(),
        ..body
    };
    if !(1..=30).contains(&new.hamlet_label.chars().count()) {
        return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
    }
    let mut tx = pool.begin().await?;
    // Locked so no RT is created under the old level while it changes.
    let old = sqlx::query!("SELECT code, hamlet_label, has_rw FROM village FOR UPDATE")
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    // With the level off every RW is inactive, so the tree stays strict.
    if old.has_rw != new.has_rw
        && sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM region_units WHERE kind = 'rt' OR kind = 'rw' AND active)
               AS "exists!""#
        )
        .fetch_one(&mut *tx)
        .await?
    {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    sqlx::query!(
        "UPDATE village SET hamlet_label = $1, has_rw = $2",
        new.hamlet_label,
        new.has_rw,
    )
    .execute(&mut *tx)
    .await?;
    let from = json!({ "hamletLabel": old.hamlet_label, "hasRw": old.has_rw });
    let changes = diff(&from, &serde_json::to_value(&new)?);
    if !changes.is_empty() {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "region.update_settings",
            ("village", Some(&old.code)),
            changes.into(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Adds a Hamlet at the end of the order, an RW to a Hamlet, or an RT to its RW (or Hamlet).
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `404` before the Village
/// Code is set, `409` for a taken number or an RW while the Village has no RW level, `422`
/// for a malformed field or a parent that is missing, inactive or of the wrong level.
#[utoipa::path(
    post,
    path = "/regions/units",
    operation_id = "createRegionUnit",
    tag = "regions",
    request_body = NewRegionUnit,
    responses(
        (status = CREATED, description = "The new Region Unit", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = NOT_FOUND, description = "The Village Code is not set yet"),
        (status = CONFLICT, description = "The number is taken, or the Village has no RW level"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed or the parent cannot take it"),
    ),
)]
pub(crate) async fn create_unit(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<NewRegionUnit>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure_manager(&session)?;
    let fields = body.fields.normalized(body.kind)?;
    let mut tx = pool.begin().await?;
    let has_rw = sqlx::query_scalar!("SELECT has_rw FROM village FOR SHARE")
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    let parent_kind = match (body.kind, has_rw) {
        (RegionKind::Hamlet, _) => None,
        (RegionKind::Rw, false) => return Err(ApiError::Status(StatusCode::CONFLICT)),
        (RegionKind::Rw, true) | (RegionKind::Rt, false) => Some(RegionKind::Hamlet),
        (RegionKind::Rt, true) => Some(RegionKind::Rw),
    };
    if let Some(parent_id) = body.parent_id {
        // Shared-locked so the parent cannot be deactivated before this child commits.
        let parent = sqlx::query!(
            r#"SELECT kind AS "kind: RegionKind", active FROM region_units WHERE id = $1
               FOR SHARE"#,
            parent_id,
        )
        .fetch_optional(&mut *tx)
        .await?;
        if !parent.is_some_and(|parent| parent.active && Some(parent.kind) == parent_kind) {
            return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
        }
    } else if parent_kind.is_some() {
        return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
    }
    let id = taken(
        sqlx::query_scalar!(
            "INSERT INTO region_units (kind, parent_id, name, sort_order, number, head_name,
             head_phone)
             SELECT $1, $2, $3, CASE WHEN $1 = 'hamlet'::region_kind
                 THEN coalesce(max(sort_order), 0) + 1 END, $4, $5, $6
             FROM region_units WHERE kind = 'hamlet'
             RETURNING id",
            body.kind as RegionKind,
            body.parent_id,
            fields.name,
            fields.number,
            fields.head_name,
            fields.head_phone,
        )
        .fetch_one(&mut *tx)
        .await,
    )?;
    let new = json!({ "kind": body.kind, "parentId": body.parent_id, "active": true });
    let mut changes = diff(&Value::Null, &new);
    changes.extend(diff(&Value::Null, &serde_json::to_value(&fields)?));
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "region.create",
        ("region_unit", Some(&id.to_string())),
        changes.into(),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

/// Renames or renumbers a Region Unit in place, edits its head, and deactivates or
/// reactivates it.
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `404` for an unknown unit,
/// `409` for a taken number, deactivating a unit with an active child, or reactivating one
/// under an inactive parent or, for an RW, while the Village has no RW level, `422` for a
/// malformed field.
#[utoipa::path(
    put,
    path = "/regions/units/{id}",
    operation_id = "updateRegionUnit",
    tag = "regions",
    params(("id" = Uuid, Path, description = "The Region Unit")),
    request_body = RegionUnitUpdate,
    responses(
        (status = NO_CONTENT, description = "The Region Unit is changed"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = NOT_FOUND, description = "There is no such Region Unit"),
        (status = CONFLICT, description = "The number is taken, the unit is not empty, or its parent is inactive"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed"),
    ),
)]
pub(crate) async fn update_unit(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<RegionUnitUpdate>,
) -> Result<StatusCode, ApiError> {
    ensure_manager(&session)?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        r#"SELECT kind AS "kind: RegionKind", parent_id, name, number, head_name, head_phone,
           active FROM region_units WHERE id = $1 FOR UPDATE"#,
        id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    let new = RegionUnitUpdate {
        fields: body.fields.normalized(old.kind)?,
        active: body.active,
    };
    let blocked = match (old.active, new.active) {
        // Children shared-lock this row when created or reactivated, so none slips in.
        (true, false) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM region_units WHERE parent_id = $1 AND active)
               AS "exists!""#,
                id,
            )
            .fetch_one(&mut *tx)
            .await?
        }
        (false, true) => {
            let parent_inactive = sqlx::query_scalar!(
                "SELECT active FROM region_units WHERE id = $1 FOR SHARE",
                old.parent_id,
            )
            .fetch_optional(&mut *tx)
            .await?
            .is_some_and(|active| !active);
            parent_inactive
                || old.kind == RegionKind::Rw
                    && !sqlx::query_scalar!("SELECT has_rw FROM village FOR SHARE")
                        .fetch_one(&mut *tx)
                        .await?
        }
        _ => false,
    };
    if blocked {
        return Err(ApiError::Status(StatusCode::CONFLICT));
    }
    taken(
        sqlx::query_scalar!(
            "UPDATE region_units SET name = $1, number = $2, head_name = $3, head_phone = $4,
             active = $5 WHERE id = $6 RETURNING id",
            new.fields.name,
            new.fields.number,
            new.fields.head_name,
            new.fields.head_phone,
            new.active,
            id,
        )
        .fetch_one(&mut *tx)
        .await,
    )?;
    let from = serde_json::to_value(RegionUnitUpdate {
        fields: RegionUnitFields {
            name: old.name,
            number: old.number,
            head_name: old.head_name,
            head_phone: old.head_phone,
        },
        active: old.active,
    })?;
    let changes = diff(&from, &serde_json::to_value(&new)?);
    if !changes.is_empty() {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "region.update",
            ("region_unit", Some(&id.to_string())),
            changes.into(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Puts every Hamlet in a new manual order.
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `422` unless the list holds
/// every Hamlet exactly once.
#[utoipa::path(
    put,
    path = "/regions/hamlet-order",
    operation_id = "orderHamlets",
    tag = "regions",
    request_body = HamletOrder,
    responses(
        (status = NO_CONTENT, description = "The Hamlets are reordered"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = UNPROCESSABLE_ENTITY, description = "The list is not every Hamlet exactly once"),
    ),
)]
pub(crate) async fn order_hamlets(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<HamletOrder>,
) -> Result<StatusCode, ApiError> {
    ensure_manager(&session)?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query_scalar!(
        "SELECT id FROM region_units WHERE kind = 'hamlet' ORDER BY sort_order, id FOR UPDATE"
    )
    .fetch_all(&mut *tx)
    .await?;
    let (mut sorted_old, mut sorted_new) = (old.clone(), body.ids.clone());
    sorted_old.sort_unstable();
    sorted_new.sort_unstable();
    if sorted_old != sorted_new {
        return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
    }
    sqlx::query!(
        "UPDATE region_units SET sort_order = ordered.sort_order
         FROM unnest($1::uuid[]) WITH ORDINALITY AS ordered (id, sort_order)
         WHERE region_units.id = ordered.id",
        &body.ids,
    )
    .execute(&mut *tx)
    .await?;
    if old != body.ids {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "region.order_hamlets",
            ("region_unit", None),
            json!([{ "field": "order", "from": old, "to": body.ids }]),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
