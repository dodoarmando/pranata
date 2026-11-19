//! The one export mechanism: sudo mode, a required reason, an Audit Entry, and the file in an
//! AES-256 ZIP under a one-time password that is shown once and never stored.

use std::io::{Cursor, Write};

use axum::http::StatusCode;
use base64::{Engine, prelude::BASE64_STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use utoipa::ToSchema;
use zip::{AesMode, ZipWriter, write::SimpleFileOptions};

use crate::{
    ApiError,
    audit::{self, Actor},
    auth::Session,
};

/// Why the User exports; required (Permenkomdigi 5/2025 Art. 93) and kept in the Audit Entry.
#[derive(Deserialize, ToSchema)]
pub struct ExportReason {
    reason: String,
}

/// An exported file, encrypted; only `password` opens it.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedFile {
    /// The ZIP's name, such as `log-audit.zip`.
    file_name: String,
    /// The one-time password, to show once; Pranata keeps no copy.
    password: String,
    /// The AES-256 ZIP holding the one file, in base64.
    content: String,
}

/// Exports `file_name` with the contents `build` makes, once the session is in sudo mode and
/// `reason` is given; the object type and the `details` object say in the Audit Entry what was
/// exported. The caller checks the Permission first.
///
/// # Errors
/// `422` for a blank reason, `428` outside sudo mode, before `build` runs.
pub(crate) async fn encrypted(
    pool: &PgPool,
    session: &Session,
    ExportReason { reason }: ExportReason,
    (object_type, mut details): (&str, Value),
    file_name: &str,
    build: impl Future<Output = Result<Vec<u8>, ApiError>>,
) -> Result<EncryptedFile, ApiError> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(ApiError::Status(StatusCode::UNPROCESSABLE_ENTITY));
    }
    session.ensure_sudo()?;
    let contents = build.await?;
    let password = one_time_password()?;
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(
        file_name,
        SimpleFileOptions::default().with_aes_encryption(AesMode::Aes256, &password),
    )?;
    zip.write_all(&contents)?;
    let content = BASE64_STANDARD.encode(zip.finish()?.into_inner());
    details["reason"] = reason.into();
    details["file"] = file_name.into();
    audit::record(
        &mut *pool.acquire().await?,
        Actor::User(session.user_id),
        &format!("{object_type}.export"),
        (object_type, None),
        details,
    )
    .await?;
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem);
    Ok(EncryptedFile {
        file_name: format!("{stem}.zip"),
        password,
        content,
    })
}

/// 20 random characters in four groups (100 bits), without look-alikes such as `0`/`o` and `1`/`l`,
/// to be read off the screen and typed into an unzip tool.
fn one_time_password() -> anyhow::Result<String> {
    const ALPHABET: &[u8; 32] = b"abcdefghijkmnpqrstuvwxyz23456789";
    let mut bytes = [0u8; 20];
    getrandom::fill(&mut bytes)?;
    let chars: Vec<char> = bytes
        .iter()
        .map(|b| char::from(ALPHABET[usize::from(b % 32)]))
        .collect();
    Ok(chars
        .chunks(5)
        .map(|group| group.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-"))
}
