//! Uploaded media: private originals, public WebP variants (D-15, D-16).
//!
//! Layout under the media directory:
//! - `originals/{hex[0..2]}/{hex}.{ext}`: private, never served; images are
//!   re-encoded without metadata once processed.
//! - `public/{hex[0..2]}/{hex}-{width}.webp`: served by Caddy at
//!   `/media/{hex[0..2]}/{hex}-{width}.webp`.
//!
//! `hex` is the SHA-256 of the uploaded bytes.

pub mod pipeline;
pub mod storage;
pub mod upload;

use std::{collections::HashMap, fmt::Write};

use anyhow::{Context, Result, anyhow};
use bytes::Bytes;
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use uuid::Uuid;

use crate::{ApiError, AppState};
pub use pipeline::Format;
pub use storage::{LocalStorage, Storage};

/// Routes for the admin media library.
#[must_use]
pub fn router() -> OpenApiRouter<AppState> {
    upload::router()
}

/// One public WebP rendition of an image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct MediaVariant {
    pub width: i32,
    /// Site-relative URL, `/media/{hex[0..2]}/{hex}-{width}.webp`.
    pub url: String,
}

/// An uploaded file as the admin API reports it.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MediaAsset {
    pub id: Uuid,
    /// `image` or `document`.
    pub kind: String,
    /// `pending` while the image job has not run, then `ready` or `failed`.
    pub status: String,
    /// Width after the EXIF orientation is applied; unknown while `pending`.
    pub width: Option<i32>,
    pub height: Option<i32>,
    /// Public renditions, narrowest first; empty until `ready`, and always
    /// empty for documents.
    pub variants: Vec<MediaVariant>,
}

/// An image as public responses embed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct MediaRef {
    pub id: Uuid,
    pub alt: Option<String>,
    pub variants: Vec<MediaVariant>,
}

/// Lower-case hexadecimal text of `bytes`.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

/// Storage path of an original.
#[must_use]
pub fn original_path(hex: &str, format: Format) -> String {
    format!("originals/{}/{hex}.{}", &hex[..2], format.extension())
}

/// Storage path of a public variant.
#[must_use]
pub fn variant_path(hex: &str, width: u32) -> String {
    format!("public/{}/{hex}-{width}.webp", &hex[..2])
}

/// The URL a variant is served at, from its storage path.
#[must_use]
pub fn variant_url(path: &str) -> String {
    format!("/media/{}", path.strip_prefix("public/").unwrap_or(path))
}

/// Load an asset with its variants.
///
/// # Errors
/// Fails when a query fails.
pub async fn load(db: &PgPool, id: Uuid) -> Result<Option<MediaAsset>> {
    let Some(row) = sqlx::query!(
        "SELECT id, kind, status, width, height FROM media_assets WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await
    .context("loading a media asset")?
    else {
        return Ok(None);
    };
    let variants = sqlx::query!(
        "SELECT width, path FROM media_variants WHERE asset_id = $1 ORDER BY width",
        id
    )
    .fetch_all(db)
    .await
    .context("loading media variants")?
    .into_iter()
    .map(|variant| MediaVariant {
        width: variant.width,
        url: variant_url(&variant.path),
    })
    .collect();
    Ok(Some(MediaAsset {
        id: row.id,
        kind: row.kind,
        status: row.status,
        width: row.width,
        height: row.height,
        variants,
    }))
}

/// Public references to the ready images among `ids`, in one query. Ids that
/// are unknown, not images, not ready or without variants are left out.
///
/// # Errors
/// Fails when the query fails.
pub async fn refs(db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, MediaRef>> {
    let rows = sqlx::query!(
        "SELECT a.id, v.width, v.path
         FROM media_assets a
         JOIN media_variants v ON v.asset_id = a.id
         WHERE a.id = ANY($1) AND a.kind = 'image' AND a.status = 'ready'
         ORDER BY a.id, v.width",
        ids
    )
    .fetch_all(db)
    .await
    .context("loading media references")?;
    let mut refs: HashMap<Uuid, MediaRef> = HashMap::new();
    for row in rows {
        refs.entry(row.id)
            .or_insert_with(|| MediaRef {
                id: row.id,
                alt: None,
                variants: Vec::new(),
            })
            .variants
            .push(MediaVariant {
                width: row.width,
                url: variant_url(&row.path),
            });
    }
    Ok(refs)
}

/// Mark a pending asset `failed`, in the caller's transaction.
///
/// # Errors
/// Fails when the update fails.
pub async fn mark_failed(conn: &mut sqlx::PgConnection, asset_id: Uuid) -> Result<()> {
    sqlx::query!(
        "UPDATE media_assets SET status = 'failed' WHERE id = $1 AND status = 'pending'",
        asset_id
    )
    .execute(conn)
    .await
    .context("marking the asset failed")?;
    Ok(())
}

/// The `ProcessImage` job: replace the raw upload with a metadata-free
/// original, write the public variants and mark the asset ready. A file the
/// pipeline rejects marks the asset `failed` without retrying and deletes the
/// raw upload, which is not metadata-free; storage and
/// database errors are returned so the job is retried.
///
/// # Errors
/// Fails when the asset is missing or storage, the database or the encoder
/// fail.
pub async fn process_asset(state: &AppState, asset_id: Uuid) -> Result<()> {
    let asset = sqlx::query!(
        "SELECT sha256, mime, status FROM media_assets WHERE id = $1 AND kind = 'image'",
        asset_id
    )
    .fetch_optional(&state.db)
    .await
    .context("loading the asset to process")?
    .ok_or_else(|| anyhow!("image asset {asset_id} does not exist"))?;
    if asset.status != "pending" {
        return Ok(());
    }
    let format = Format::from_mime(&asset.mime)
        .filter(|format| format.is_image())
        .ok_or_else(|| anyhow!("asset {asset_id} has the non-image type {}", asset.mime))?;
    let hex = hex(&asset.sha256);
    let original_key = original_path(&hex, format);
    let raw: Bytes = state.storage.get(&original_key).await?;

    let processed = tokio::task::spawn_blocking(move || pipeline::process_image(&raw))
        .await
        .context("image processing task")?;
    let processed = match processed {
        Ok(processed) => processed,
        Err(ApiError::Internal(error)) => return Err(error),
        Err(rejected) => {
            tracing::warn!(%asset_id, error = %rejected, "image rejected by the pipeline");
            // The raw upload may carry GPS and other metadata. A re-upload
            // brings its bytes again, so nothing needs keeping. Deleting first
            // means a crash here retries the job, which then fails to read the
            // original and ends `failed` after its attempts.
            state.storage.delete(&original_key).await?;
            sqlx::query!(
                "UPDATE media_assets SET status = 'failed' WHERE id = $1",
                asset_id
            )
            .execute(&state.db)
            .await
            .context("marking the asset failed")?;
            return Ok(());
        }
    };

    state
        .storage
        .put(&original_key, Bytes::from(processed.original))
        .await?;
    let mut stored = Vec::with_capacity(processed.variants.len());
    for variant in processed.variants {
        let path = variant_path(&hex, variant.width);
        let size = i64::try_from(variant.webp.len()).context("variant size")?;
        state.storage.put(&path, Bytes::from(variant.webp)).await?;
        stored.push((i32::try_from(variant.width)?, path, size));
    }

    let mut tx = state.db.begin().await.context("starting a transaction")?;
    for (width, path, size) in stored {
        sqlx::query!(
            "INSERT INTO media_variants (asset_id, width, path, bytes) VALUES ($1, $2, $3, $4)
             ON CONFLICT (asset_id, width) DO UPDATE SET path = EXCLUDED.path, bytes = EXCLUDED.bytes",
            asset_id,
            width,
            path,
            size,
        )
        .execute(&mut *tx)
        .await
        .context("recording a variant")?;
    }
    sqlx::query!(
        "UPDATE media_assets SET status = 'ready', width = $2, height = $3 WHERE id = $1",
        asset_id,
        i32::try_from(processed.width)?,
        i32::try_from(processed.height)?,
    )
    .execute(&mut *tx)
    .await
    .context("marking the asset ready")?;
    tx.commit()
        .await
        .context("committing the processed asset")?;
    Ok(())
}
