//! Where uploaded files live (D-15). Everything goes through [`Storage`], so an
//! object store can replace the local volume later.

use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use bytes::Bytes;
use uuid::Uuid;

/// A key-value store for files, addressed by a relative path such as
/// `originals/ab/abcd….jpg`.
#[async_trait]
pub trait Storage: Send + Sync {
    /// Store `bytes` under `rel`, replacing any file already there. A reader
    /// never sees a half-written file.
    ///
    /// # Errors
    /// Fails when `rel` is not a plain relative path or the write fails.
    async fn put(&self, rel: &str, bytes: Bytes) -> Result<()>;

    /// Read the file stored under `rel`.
    ///
    /// # Errors
    /// Fails when `rel` is not a plain relative path or the file is missing.
    async fn get(&self, rel: &str) -> Result<Bytes>;

    /// Remove the file stored under `rel`. A file that is already gone is not
    /// an error, so a retried cleanup succeeds.
    ///
    /// # Errors
    /// Fails when `rel` is not a plain relative path or the removal fails.
    async fn delete(&self, rel: &str) -> Result<()>;
}

/// Files on a local directory. Caddy serves its `public/` subdirectory at
/// `/media/`; `originals/` is never exposed.
#[derive(Debug, Clone)]
pub struct LocalStorage {
    root: PathBuf,
}

impl LocalStorage {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn resolve(&self, rel: &str) -> Result<PathBuf> {
        let path = Path::new(rel);
        let plain = path.components().count() > 0
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_)));
        if !plain {
            bail!("storage path {rel:?} must be relative and stay inside the media directory");
        }
        Ok(self.root.join(path))
    }
}

#[async_trait]
impl Storage for LocalStorage {
    async fn put(&self, rel: &str, bytes: Bytes) -> Result<()> {
        let path = self.resolve(rel)?;
        let dir = path.parent().context("storage path has a parent")?;
        tokio::fs::create_dir_all(dir)
            .await
            .with_context(|| format!("creating {}", dir.display()))?;
        // Write beside the target and rename, so the file appears whole.
        let temp = dir.join(format!(".{}.tmp", Uuid::new_v4()));
        tokio::fs::write(&temp, &bytes)
            .await
            .with_context(|| format!("writing {}", temp.display()))?;
        if let Err(error) = tokio::fs::rename(&temp, &path).await {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(error).with_context(|| format!("moving into {}", path.display()));
        }
        Ok(())
    }

    async fn get(&self, rel: &str) -> Result<Bytes> {
        let path = self.resolve(rel)?;
        let bytes = tokio::fs::read(&path)
            .await
            .with_context(|| format!("reading {}", path.display()))?;
        Ok(Bytes::from(bytes))
    }

    async fn delete(&self, rel: &str) -> Result<()> {
        let path = self.resolve(rel)?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).with_context(|| format!("removing {}", path.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage() -> LocalStorage {
        LocalStorage::new(std::env::temp_dir().join(format!("pranata-storage-{}", Uuid::new_v4())))
    }

    #[tokio::test]
    async fn put_then_get_round_trips_and_replaces() {
        let storage = storage();
        storage
            .put("a/b/c.bin", Bytes::from_static(b"one"))
            .await
            .unwrap();
        storage
            .put("a/b/c.bin", Bytes::from_static(b"two"))
            .await
            .unwrap();
        assert_eq!(
            storage.get("a/b/c.bin").await.unwrap(),
            Bytes::from_static(b"two")
        );
        let leftovers = std::fs::read_dir(storage.root.join("a/b")).unwrap().count();
        assert_eq!(leftovers, 1, "no temp file may remain");
    }

    #[tokio::test]
    async fn delete_removes_the_file_and_tolerates_a_missing_one() {
        let storage = storage();
        storage
            .put("a/b.bin", Bytes::from_static(b"x"))
            .await
            .unwrap();
        storage.delete("a/b.bin").await.unwrap();
        assert!(storage.get("a/b.bin").await.is_err());
        storage.delete("a/b.bin").await.unwrap();
    }

    #[tokio::test]
    async fn paths_cannot_leave_the_root() {
        let storage = storage();
        for rel in ["../x", "/etc/passwd", "a/../../x", "", "."] {
            assert!(storage.put(rel, Bytes::new()).await.is_err(), "{rel:?}");
            assert!(storage.get(rel).await.is_err(), "{rel:?}");
            assert!(storage.delete(rel).await.is_err(), "{rel:?}");
        }
    }

    #[tokio::test]
    async fn missing_files_are_errors() {
        assert!(storage().get("nope.bin").await.is_err());
    }
}
