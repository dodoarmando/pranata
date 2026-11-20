//! Files on the Installation's local volume. Each file is a directory named after its row in
//! `files`, written once; the row is what an owner, such as the Village logo, references.

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::Context;
use axum::{
    extract::{self, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use image::DynamicImage;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    ApiError,
    imaging::{self, Kind, Rejected, Variant},
};

/// The volume's root directory.
pub struct Storage {
    dir: PathBuf,
}

impl Storage {
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Stores an upload of `kind` as an image: its master and every [`Variant`].
    ///
    /// The file is unreferenced until an owner points to it, and swept if none does in 24 h.
    pub(crate) async fn save_image(
        &self,
        pool: &PgPool,
        kind: Kind,
        upload: Vec<u8>,
    ) -> Result<Uuid, ApiError> {
        let image = tokio::task::spawn_blocking(move || imaging::decode(kind, &upload))
            .await?
            .map_err(|rejected| {
                ApiError::Status(match rejected {
                    Rejected::UnsupportedType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    Rejected::TooManyPixels | Rejected::Corrupt => StatusCode::UNPROCESSABLE_ENTITY,
                })
            })?;
        let mut tx = pool.begin().await?;
        let id = sqlx::query_scalar!("INSERT INTO files DEFAULT VALUES RETURNING id")
            .fetch_one(&mut *tx)
            .await?;
        let dir = self.dir.clone();
        tokio::task::spawn_blocking(move || write_image(&dir, id, &image)).await??;
        tx.commit().await?;
        Ok(id)
    }
}

/// Deletes every file no owner references once its `sweep_after` has passed, and returns how
/// many went.
///
/// # Errors
/// When the database is unreachable or a file's directory cannot be removed.
pub async fn sweep(pool: &PgPool, dir: &Path) -> anyhow::Result<usize> {
    // Each owner adds its reference here; the foreign keys refuse a delete one forgets.
    let ids = sqlx::query_scalar!(
        "DELETE FROM files WHERE sweep_after < now()
         AND id NOT IN (SELECT logo_file_id FROM village WHERE logo_file_id IS NOT NULL)
         AND id NOT IN (SELECT photo_file_id FROM tenures WHERE photo_file_id IS NOT NULL)
         AND id NOT IN (SELECT logo_file_id FROM institutions WHERE logo_file_id IS NOT NULL)
         AND id NOT IN (SELECT cover_file_id FROM news_posts WHERE cover_file_id IS NOT NULL)
         AND id NOT IN (SELECT cover_file_id FROM pages WHERE cover_file_id IS NOT NULL)
         RETURNING id"
    )
    .fetch_all(pool)
    .await?;
    for id in &ids {
        match tokio::fs::remove_dir_all(dir.join(id.to_string())).await {
            Err(error) if error.kind() != ErrorKind::NotFound => {
                return Err(error).with_context(|| format!("cannot remove file {id}"));
            }
            _ => {}
        }
    }
    Ok(ids.len())
}

/// Starts the 24 h before a file its owner no longer references is swept; call it in the
/// transaction that drops the reference.
///
/// # Errors
/// When the update fails, which must abort the change.
pub(crate) async fn release(conn: &mut sqlx::PgConnection, id: Uuid) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE files SET sweep_after = now() + interval '24 hours' WHERE id = $1",
        id
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Rebuilds every variant of every image from its master, such as after the variants change,
/// and returns how many images it rebuilt.
///
/// # Errors
/// When the volume cannot be read or written, or a master does not decode.
pub fn regenerate(dir: &Path) -> anyhow::Result<usize> {
    let mut rebuilt = 0;
    for entry in std::fs::read_dir(dir)? {
        let file = entry?.path();
        // Skips staging directories and files that are not images.
        let is_file = file
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.parse::<Uuid>().is_ok());
        let master = ["master.png", "master.jpg"]
            .map(|name| file.join(name))
            .into_iter()
            .find(|master| master.exists());
        let (true, Some(master)) = (is_file, master) else {
            continue;
        };
        let image =
            image::open(&master).with_context(|| format!("cannot decode {}", master.display()))?;
        for variant in Variant::ALL {
            let staged = file.join(format!(".{}", variant.file_name()));
            std::fs::write(&staged, imaging::render(&image, variant)?)?;
            std::fs::rename(&staged, file.join(variant.file_name()))?;
        }
        rebuilt += 1;
    }
    Ok(rebuilt)
}

/// Writes the master and variants beside each other, then moves them into place at once;
/// the move fails rather than replace an existing file.
fn write_image(dir: &Path, id: Uuid, image: &DynamicImage) -> anyhow::Result<()> {
    let staging = dir.join(format!(".{id}"));
    std::fs::create_dir(&staging)
        .with_context(|| format!("cannot create {}", staging.display()))?;
    let (master, extension) = imaging::encode_master(image)?;
    std::fs::write(staging.join(format!("master.{extension}")), master)?;
    for variant in Variant::ALL {
        std::fs::write(
            staging.join(variant.file_name()),
            imaging::render(image, variant)?,
        )?;
    }
    std::fs::rename(&staging, dir.join(id.to_string()))
        .with_context(|| format!("cannot move file {id} into place"))
}

/// An image's public rendition: `480`, `960` or `1600` (WebP) or `share` (JPEG). Cached
/// forever, since a file never changes; only images that hold no personal data come here.
///
/// # Errors
/// `404` for an unknown file or variant.
#[utoipa::path(
    get,
    path = "/media/{id}/{variant}",
    operation_id = "getMedia",
    tag = "media",
    params(
        ("id" = Uuid, Path, description = "The file"),
        ("variant" = String, Path, description = "`480`, `960`, `1600` or `share`"),
    ),
    responses(
        (status = OK, description = "The image", content(("image/webp"), ("image/jpeg"))),
        (status = NOT_FOUND, description = "There is no such file or variant"),
    ),
)]
pub(crate) async fn get_media(
    State(storage): State<std::sync::Arc<Storage>>,
    extract::Path((id, name)): extract::Path<(Uuid, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let variant = Variant::ALL
        .into_iter()
        .find(|variant| variant.name() == name)
        .ok_or(ApiError::Status(StatusCode::NOT_FOUND))?;
    let path = storage.dir.join(id.to_string()).join(variant.file_name());
    let bytes = match tokio::fs::read(&path).await {
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(ApiError::Status(StatusCode::NOT_FOUND));
        }
        bytes => bytes?,
    };
    Ok((
        [
            (header::CONTENT_TYPE, variant.content_type()),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        bytes,
    ))
}
