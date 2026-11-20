//! Institutions: bodies outside the Village government and their ordered members.

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
    government::{Created, ensure_manager, known},
    storage,
    village::diff,
};

/// One member of an Institution; members hold no Tenure and sign no letter.
#[derive(Serialize, Deserialize, ToSchema, Debug, PartialEq, Eq)]
pub struct Member {
    /// 1–100 characters.
    name: String,
    /// Such as Ketua or Anggota; 1–100 characters.
    role: String,
}

/// What a User records about an Institution.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct InstitutionFields {
    /// 1–100 characters.
    name: String,
    /// Such as BPD; up to 30 characters.
    #[serde(default)]
    abbreviation: Option<String>,
    /// 1–2000 characters.
    description: String,
    /// The logo's file, uploaded at `/village/logo` and served at `/media/{logo}/{variant}`.
    #[serde(default)]
    logo: Option<Uuid>,
    /// In order; at most 100.
    members: Vec<Member>,
}

impl InstitutionFields {
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
        if self.members.len() > 100 {
            return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
        }
        Ok(Self {
            name: required(self.name, 100)?,
            abbreviation: text(self.abbreviation, 30)?,
            description: required(self.description, 2000)?,
            logo: self.logo,
            members: self
                .members
                .into_iter()
                .map(|member| {
                    Ok(Member {
                        name: required(member.name, 100)?,
                        role: required(member.role, 100)?,
                    })
                })
                .collect::<Result<_, ApiError>>()?,
        })
    }
}

#[derive(Serialize, ToSchema)]
pub struct Institution {
    id: Uuid,
    #[serde(flatten)]
    fields: InstitutionFields,
}

/// Replaces an Institution's members with `members`, in their order.
async fn save_members(
    conn: &mut sqlx::PgConnection,
    id: Uuid,
    members: &[Member],
) -> sqlx::Result<()> {
    sqlx::query!(
        "DELETE FROM institution_members WHERE institution_id = $1",
        id
    )
    .execute(&mut *conn)
    .await?;
    let (names, roles): (Vec<&str>, Vec<&str>) = members
        .iter()
        .map(|member| (member.name.as_str(), member.role.as_str()))
        .unzip();
    sqlx::query!(
        "INSERT INTO institution_members (institution_id, sort_order, name, role)
         SELECT $1, sort_order::integer, name, role
         FROM unnest($2::text[], $3::text[]) WITH ORDINALITY AS member (name, role, sort_order)",
        id,
        &names as &[&str],
        &roles as &[&str],
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Every Institution by name with its members, for the public site and the admin area.
#[utoipa::path(
    get,
    path = "/institutions",
    operation_id = "listInstitutions",
    tag = "institution",
    responses((status = OK, description = "Every Institution", body = Vec<Institution>)),
)]
pub(crate) async fn list(State(pool): State<PgPool>) -> Result<Json<Vec<Institution>>, ApiError> {
    let mut members = sqlx::query!(
        "SELECT institution_id, name, role FROM institution_members
         ORDER BY institution_id, sort_order"
    )
    .fetch_all(&pool)
    .await?;
    let institutions = sqlx::query!(
        "SELECT id, name, abbreviation, description, logo_file_id FROM institutions
         ORDER BY name, id"
    )
    .fetch_all(&pool)
    .await?
    .into_iter()
    .map(|row| Institution {
        fields: InstitutionFields {
            members: members
                .extract_if(.., |member| member.institution_id == row.id)
                .map(|member| Member {
                    name: member.name,
                    role: member.role,
                })
                .collect(),
            name: row.name,
            abbreviation: row.abbreviation,
            description: row.description,
            logo: row.logo_file_id,
        },
        id: row.id,
    })
    .collect();
    Ok(Json(institutions))
}

/// Records an Institution and its members.
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `422` for a blank name or
/// description, a malformed field, a blank member, over 100 members, or an unknown logo.
#[utoipa::path(
    post,
    path = "/institutions",
    operation_id = "createInstitution",
    tag = "institution",
    request_body = InstitutionFields,
    responses(
        (status = CREATED, description = "The new Institution", body = Created),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed or unknown"),
    ),
)]
pub(crate) async fn create(
    State(pool): State<PgPool>,
    session: Session,
    Json(body): Json<InstitutionFields>,
) -> Result<(StatusCode, Json<Created>), ApiError> {
    ensure_manager(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let id = known(
        sqlx::query_scalar!(
            "INSERT INTO institutions (name, abbreviation, description, logo_file_id)
             VALUES ($1, $2, $3, $4) RETURNING id",
            new.name,
            new.abbreviation,
            new.description,
            new.logo,
        )
        .fetch_one(&mut *tx)
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
    )?;
    save_members(&mut tx, id, &new.members).await?;
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "institution.create",
        ("institution", Some(&id.to_string())),
        diff(&Value::Null, &serde_json::to_value(&new)?).into(),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Created { id })))
}

/// Changes an Institution and replaces its members.
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `404` for an unknown
/// Institution, `422` for a blank name or description, a malformed field, a blank member, over 100 members, or an unknown
/// logo.
#[utoipa::path(
    put,
    path = "/institutions/{id}",
    operation_id = "updateInstitution",
    tag = "institution",
    params(("id" = Uuid, Path, description = "The Institution")),
    request_body = InstitutionFields,
    responses(
        (status = NO_CONTENT, description = "The Institution is changed"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = NOT_FOUND, description = "There is no such Institution"),
        (status = UNPROCESSABLE_ENTITY, description = "A field is malformed or unknown"),
    ),
)]
pub(crate) async fn update(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
    Json(body): Json<InstitutionFields>,
) -> Result<StatusCode, ApiError> {
    ensure_manager(&session)?;
    let new = body.normalized()?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        "SELECT name, abbreviation, description, logo_file_id FROM institutions
         WHERE id = $1 FOR UPDATE",
        id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    let members = sqlx::query_as!(
        Member,
        "SELECT name, role FROM institution_members WHERE institution_id = $1 ORDER BY sort_order",
        id,
    )
    .fetch_all(&mut *tx)
    .await?;
    known(
        sqlx::query!(
            "UPDATE institutions SET name = $1, abbreviation = $2, description = $3,
             logo_file_id = $4 WHERE id = $5",
            new.name,
            new.abbreviation,
            new.description,
            new.logo,
            id,
        )
        .execute(&mut *tx)
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
    )?;
    if members != new.members {
        save_members(&mut tx, id, &new.members).await?;
    }
    if let Some(logo) = old.logo_file_id.filter(|&logo| Some(logo) != new.logo) {
        storage::release(&mut tx, logo).await?;
    }
    let from = serde_json::to_value(InstitutionFields {
        name: old.name,
        abbreviation: old.abbreviation,
        description: old.description,
        logo: old.logo_file_id,
        members,
    })?;
    let changes = diff(&from, &serde_json::to_value(&new)?);
    if !changes.is_empty() {
        audit::record_change(
            &mut tx,
            Actor::User(session.user_id),
            "institution.update",
            ("institution", Some(&id.to_string())),
            changes.into(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Deletes an Institution and its members.
///
/// # Errors
/// `401` without a live session, `403` without `village.manage`, `404` for an unknown
/// Institution.
#[utoipa::path(
    delete,
    path = "/institutions/{id}",
    operation_id = "deleteInstitution",
    tag = "institution",
    params(("id" = Uuid, Path, description = "The Institution")),
    responses(
        (status = NO_CONTENT, description = "The Institution is deleted"),
        (status = UNAUTHORIZED, description = "There is no live session"),
        (status = FORBIDDEN, description = "The User may not manage the Village"),
        (status = NOT_FOUND, description = "There is no such Institution"),
    ),
)]
pub(crate) async fn delete(
    State(pool): State<PgPool>,
    session: Session,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    ensure_manager(&session)?;
    let mut tx = pool.begin().await?;
    let old = sqlx::query!(
        "DELETE FROM institutions WHERE id = $1 RETURNING name, logo_file_id",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    if let Some(logo) = old.logo_file_id {
        storage::release(&mut tx, logo).await?;
    }
    audit::record_change(
        &mut tx,
        Actor::User(session.user_id),
        "institution.delete",
        ("institution", Some(&id.to_string())),
        json!([{ "field": "name", "from": old.name, "to": null }]),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
