use std::path::{Path, PathBuf};

use bytes::Bytes;

use super::StorageError;

/// Local filesystem storage backend.
///
/// Files are stored under `base_path` using the storage key as the relative
/// path. Content type is inferred from the file extension on reads.
#[derive(Debug, Clone)]
pub struct LocalStorage {
    base_path: PathBuf,
}

impl LocalStorage {
    pub fn new(base_path: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(&base_path)?;
        Ok(Self { base_path })
    }

    fn resolve(&self, key: &str) -> PathBuf {
        self.base_path.join(key)
    }

    pub async fn put(
        &self,
        key: &str,
        data: Bytes,
        _content_type: &str,
    ) -> Result<(), StorageError> {
        let path = self.resolve(key);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, &data).await?;
        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<(Bytes, String), StorageError> {
        let path = self.resolve(key);
        if !path.exists() {
            return Err(StorageError::NotFound(key.to_string()));
        }
        let data = tokio::fs::read(&path).await?;
        let content_type = guess_content_type(&path);
        Ok((Bytes::from(data), content_type))
    }

    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        let path = self.resolve(key);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

fn guess_content_type(path: &Path) -> String {
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn put_and_get_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let storage = LocalStorage::new(dir.path().to_path_buf()).unwrap();

        storage
            .put("test/file.txt", Bytes::from("hello"), "text/plain")
            .await
            .unwrap();

        let (data, ct) = storage.get("test/file.txt").await.unwrap();
        assert_eq!(data, Bytes::from("hello"));
        assert_eq!(ct, "text/plain");
    }

    #[tokio::test]
    async fn delete_removes_file() {
        let dir = tempfile::tempdir().unwrap();
        let storage = LocalStorage::new(dir.path().to_path_buf()).unwrap();

        storage
            .put("d.txt", Bytes::from("data"), "text/plain")
            .await
            .unwrap();
        storage.delete("d.txt").await.unwrap();

        assert!(matches!(
            storage.get("d.txt").await,
            Err(StorageError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn get_nonexistent_returns_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let storage = LocalStorage::new(dir.path().to_path_buf()).unwrap();

        assert!(matches!(
            storage.get("nope.txt").await,
            Err(StorageError::NotFound(_))
        ));
    }
}
