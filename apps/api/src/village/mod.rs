//! The Installation's single Village: identity taken from the bundled region
//! dataset (D-02).

use anyhow::Context;
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::audit::{self, AuditAction, NewAuditEntry};
use crate::region_codes::{self, TimeZone, VillageCode};

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
    })
}

/// The Village profile, or `None` before `init`.
///
/// # Errors
/// Fails when the query fails or the stored time zone is unknown.
pub async fn get(pool: &PgPool) -> anyhow::Result<Option<VillageProfile>> {
    let row = sqlx::query!(
        "SELECT code, name, district, regency, province, time_zone FROM village WHERE id = 1"
    )
    .fetch_optional(pool)
    .await
    .context("reading village")?;
    row.map(|row| {
        let zone = TimeZone::from_iana_name(&row.time_zone)
            .with_context(|| format!("unknown stored time zone {}", row.time_zone))?;
        Ok(VillageProfile {
            code: row.code,
            name: row.name,
            district: row.district,
            regency: row.regency,
            province: row.province,
            time_zone: zone.iana_name().to_owned(),
        })
    })
    .transpose()
}
