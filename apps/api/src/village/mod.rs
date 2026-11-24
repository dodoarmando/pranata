//! The Installation's single Village: identity taken from the bundled region
//! dataset (D-02).

pub mod profile;

use std::collections::{HashMap, HashSet};

use anyhow::Context;
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::audit::{self, AuditAction, NewAuditEntry};
use crate::media::{self, MediaRef};
use crate::region_codes::{self, TimeZone, VillageCode};
use crate::rich_text;

/// What the public site shows about the Village.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct VillageProfile {
    /// Dotted 10-digit village code.
    pub code: String,
    pub name: String,
    pub district: String,
    pub regency: String,
    pub province: String,
    /// IANA time zone name.
    pub time_zone: String,
    /// The overview as sanitized HTML; `null` until an Administrator writes
    /// one.
    pub overview_html: Option<String>,
    /// Hero photo; `null` until an Administrator picks one.
    pub hero: Option<MediaRef>,
}

/// Why `init` failed.
#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("the village code is not in the bundled region dataset")]
    UnknownCode,
    #[error("a village is already initialised")]
    AlreadyInitialised,
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

/// Create the single Village from its code, with an audit entry in the same
/// transaction.
///
/// # Errors
/// `UnknownCode` when the dataset has no such village, `AlreadyInitialised`
/// when the row exists, `Internal` on database failures.
pub async fn init(pool: &PgPool, code: &VillageCode) -> Result<VillageProfile, InitError> {
    let record = region_codes::lookup(code).ok_or(InitError::UnknownCode)?;
    let mut tx = pool.begin().await.context("starting transaction")?;
    let inserted = sqlx::query_scalar!(
        r#"INSERT INTO village (id, code, name, district, regency, province, time_zone)
           VALUES (1, $1, $2, $3, $4, $5, $6)
           ON CONFLICT (id) DO NOTHING
           RETURNING id"#,
        code.as_str(),
        record.village,
        record.district,
        record.regency,
        record.province,
        record.time_zone.iana_name(),
    )
    .fetch_optional(&mut *tx)
    .await
    .context("inserting village")?;
    if inserted.is_none() {
        return Err(InitError::AlreadyInitialised);
    }
    audit::record(
        &mut tx,
        NewAuditEntry {
            actor: None,
            action: AuditAction::Create,
            object_type: "village",
            object_id: code.to_string(),
            summary: format!("Village initialised from code {code}"),
            changes: Vec::new(),
        },
    )
    .await?;
    tx.commit().await.context("committing village init")?;
    Ok(VillageProfile {
        code: code.to_string(),
        name: record.village,
        district: record.district,
        regency: record.regency,
        province: record.province,
        time_zone: record.time_zone.iana_name().to_owned(),
        overview_html: None,
        hero: None,
    })
}

/// Ids of the images a rich text document embeds.
fn image_ids(node: &Value, ids: &mut HashSet<Uuid>) {
    if node.get("type").and_then(Value::as_str) == Some("image")
        && let Some(id) = node
            .get("attrs")
            .and_then(|attrs| attrs.get("assetId"))
            .and_then(Value::as_str)
            .and_then(|id| Uuid::parse_str(id).ok())
    {
        ids.insert(id);
    }
    for child in node
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        image_ids(child, ids);
    }
}

/// Load every image the profile shows (the hero and the ones in `overview`)
/// with a single query.
async fn load_images(
    pool: &PgPool,
    overview: Option<&Value>,
    hero: Option<Uuid>,
) -> anyhow::Result<HashMap<Uuid, MediaRef>> {
    let mut ids = HashSet::new();
    ids.extend(hero);
    if let Some(overview) = overview {
        image_ids(overview, &mut ids);
    }
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids: Vec<Uuid> = ids.into_iter().collect();
    media::refs(pool, &ids).await
}

/// The Village profile, or `None` before `init`.
///
/// # Errors
/// Fails when the query fails or the stored time zone is unknown.
pub async fn get(pool: &PgPool) -> anyhow::Result<Option<VillageProfile>> {
    let row = sqlx::query!(
        "SELECT code, name, district, regency, province, time_zone, overview, hero_asset_id
         FROM village WHERE id = 1"
    )
    .fetch_optional(pool)
    .await
    .context("reading village")?;
    let Some(row) = row else {
        return Ok(None);
    };
    let zone = TimeZone::from_iana_name(&row.time_zone)
        .with_context(|| format!("unknown stored time zone {}", row.time_zone))?;
    let images = load_images(pool, row.overview.as_ref(), row.hero_asset_id).await?;
    // Stored documents were sanitized on write; validating again is cheap
    // and keeps `to_html` fed with a `RichTextDoc` only.
    let overview_html = row
        .overview
        .as_ref()
        .map(|value| {
            let doc = rich_text::validate(value).context("stored overview is invalid")?;
            Ok::<_, anyhow::Error>(rich_text::to_html(&doc, &|id| images.get(&id).cloned()))
        })
        .transpose()?;
    Ok(Some(VillageProfile {
        code: row.code,
        name: row.name,
        district: row.district,
        regency: row.regency,
        province: row.province,
        time_zone: zone.iana_name().to_owned(),
        overview_html,
        hero: row.hero_asset_id.and_then(|id| images.get(&id).cloned()),
    }))
}
