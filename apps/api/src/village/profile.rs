//! Editing the Village overview and hero photo (D-12, D-35).

use anyhow::Context;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    ApiError, FieldError,
    audit::{self, AuditAction, NewAuditEntry},
    ids::UserId,
    jobs::{self, Job},
    media::{self, MediaRef},
    rich_text::{self, RichTextError},
};

/// What an Administrator edits.
#[derive(Debug, Serialize, ToSchema)]
pub struct Profile {
    /// `ProseMirror` document; `null` when there is no overview.
    #[schema(value_type = Option<Object>)]
    pub overview: Option<Value>,
    pub hero: Option<MediaRef>,
    /// Opaque value; send back exactly as received.
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: OffsetDateTime,
}

/// Keeps a missing key an error: `null` clears a field, absence is a mistake.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer)
}

/// The edit an Administrator submits.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ProfileUpdate {
    /// New overview, or `null` to clear it.
    #[serde(deserialize_with = "present")]
    #[schema(value_type = Option<Object>, required = true)]
    pub overview: Option<Value>,
    /// A ready image asset, or `null` for no hero photo.
    #[serde(deserialize_with = "present")]
    #[schema(required = true)]
    pub hero_asset_id: Option<Uuid>,
    /// Opaque value; send back exactly as received.
    #[serde(with = "time::serde::rfc3339")]
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: OffsetDateTime,
}

/// The audited fields.
#[derive(Serialize)]
struct Tracked {
    overview: Option<Value>,
    hero_asset_id: Option<Uuid>,
}

async fn hero_ref(db: &PgPool, hero: Option<Uuid>) -> anyhow::Result<Option<MediaRef>> {
    let Some(id) = hero else {
        return Ok(None);
    };
    Ok(media::refs(db, &[id]).await?.remove(&id))
}

/// The profile as the editor sees it.
///
/// # Errors
/// 404 `village_not_initialized` before `pranata village init` has run.
pub async fn get(db: &PgPool) -> Result<Profile, ApiError> {
    let row = sqlx::query!("SELECT overview, hero_asset_id, updated_at FROM village WHERE id = 1")
        .fetch_optional(db)
        .await
        .context("reading the village profile")?
        .ok_or(ApiError::NotFound("village_not_initialized"))?;
    Ok(Profile {
        hero: hero_ref(db, row.hero_asset_id).await?,
        overview: row.overview,
        updated_at: row.updated_at,
    })
}

fn overview_error(error: RichTextError) -> ApiError {
    let code = match error {
        RichTextError::NotADocument => "overview_invalid",
        RichTextError::TooLarge => "overview_too_large",
    };
    ApiError::Validation(vec![FieldError {
        field: "overview".to_owned(),
        code: code.to_owned(),
    }])
}

/// Replace the overview and hero photo. One transaction holds the update, its
/// Audit Entry and the cache purge job.
///
/// # Errors
/// 422 for an invalid overview (`overview_invalid`, `overview_too_large`) or a
/// hero that is not a ready image (`hero_not_ready`); 409 `stale_write` when
/// `updated_at` is not the stored one; 404 `village_not_initialized`.
pub async fn update(db: &PgPool, actor: UserId, input: ProfileUpdate) -> Result<Profile, ApiError> {
    let overview = input
        .overview
        .as_ref()
        .map(|value| rich_text::validate(value).map(rich_text::RichTextDoc::into_value))
        .transpose()
        .map_err(overview_error)?;

    let mut tx = db.begin().await.context("starting transaction")?;
    // Lock the row so the audited "before" is the state this update replaces.
    let before =
        sqlx::query!("SELECT overview, hero_asset_id FROM village WHERE id = 1 FOR UPDATE")
            .fetch_optional(&mut *tx)
            .await
            .context("reading the village profile")?
            .ok_or(ApiError::NotFound("village_not_initialized"))?;

    if let Some(hero) = input.hero_asset_id {
        let ready = sqlx::query_scalar!(
            r#"SELECT EXISTS (
                   SELECT 1 FROM media_assets
                   WHERE id = $1 AND kind = 'image' AND status = 'ready'
               ) AS "ready!""#,
            hero
        )
        .fetch_one(&mut *tx)
        .await
        .context("checking the hero asset")?;
        if !ready {
            return Err(ApiError::Unprocessable("hero_not_ready"));
        }
    }

    let updated_at = sqlx::query_scalar!(
        "UPDATE village
         SET overview = $1, hero_asset_id = $2, updated_at = clock_timestamp()
         WHERE id = 1 AND updated_at = $3
         RETURNING updated_at",
        overview,
        input.hero_asset_id,
        input.updated_at,
    )
    .fetch_optional(&mut *tx)
    .await
    .context("updating the village profile")?
    .ok_or(ApiError::Conflict("stale_write"))?;

    let changes = audit::diff(
        &Tracked {
            overview: before.overview,
            hero_asset_id: before.hero_asset_id,
        },
        &Tracked {
            overview: overview.clone(),
            hero_asset_id: input.hero_asset_id,
        },
    );
    audit::record(
        &mut tx,
        NewAuditEntry {
            actor: Some(actor),
            action: AuditAction::Update,
            object_type: "village",
            object_id: "1".to_owned(),
            summary: "Village profile updated".to_owned(),
            changes,
        },
    )
    .await?;
    jobs::enqueue(&mut tx, Job::PurgePublicCache, OffsetDateTime::now_utc()).await?;
    tx.commit().await.context("committing the profile update")?;

    Ok(Profile {
        hero: hero_ref(db, input.hero_asset_id).await?,
        overview,
        updated_at,
    })
}
