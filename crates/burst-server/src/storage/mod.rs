pub mod local;

use bytes::Bytes;

/// Enum-based storage dispatch (avoids trait objects while keeping extensibility).
/// Add variants here for future backends (e.g. `Gateway` for S3 via Barbacane).
#[derive(Clone)]
pub enum Storage {
    Local(local::LocalStorage),
}

impl Storage {
    pub async fn put(
        &self,
        key: &str,
        data: Bytes,
        content_type: &str,
    ) -> Result<(), StorageError> {
        match self {
            Storage::Local(s) => s.put(key, data, content_type).await,
        }
    }

    pub async fn get(&self, key: &str) -> Result<(Bytes, String), StorageError> {
        match self {
            Storage::Local(s) => s.get(key).await,
        }
    }

    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        match self {
            Storage::Local(s) => s.delete(key).await,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not found: {0}")]
    NotFound(String),
}
