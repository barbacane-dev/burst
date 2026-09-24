pub mod gateway;
pub mod local;

use std::path::Path;

use bytes::Bytes;
use futures_util::stream::BoxStream;

/// An object's content, read in chunks.
pub type ByteStream = BoxStream<'static, Result<Bytes, std::io::Error>>;

/// Enum-based storage dispatch.
/// `Local` for filesystem, `Gateway` for S3 via Barbacane (ADR-011).
#[derive(Clone)]
pub enum Storage {
    Local(local::LocalStorage),
    Gateway(gateway::GatewayStorage),
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
            Storage::Gateway(s) => s.put(key, data, content_type).await,
        }
    }

    pub async fn get(&self, key: &str) -> Result<(Bytes, String), StorageError> {
        match self {
            Storage::Local(s) => s.get(key).await,
            Storage::Gateway(s) => s.get(key).await,
        }
    }

    /// Stores the file at `path` without reading it into memory.
    pub async fn put_file(
        &self,
        key: &str,
        path: &Path,
        content_type: &str,
    ) -> Result<(), StorageError> {
        match self {
            Storage::Local(s) => s.put_file(key, path).await,
            Storage::Gateway(s) => s.put_file(key, path, content_type).await,
        }
    }

    /// Reads an object as a stream, with its length when known.
    pub async fn get_stream(&self, key: &str) -> Result<(ByteStream, Option<u64>), StorageError> {
        match self {
            Storage::Local(s) => s.get_stream(key).await,
            Storage::Gateway(s) => s.get_stream(key).await,
        }
    }

    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        match self {
            Storage::Local(s) => s.delete(key).await,
            Storage::Gateway(s) => s.delete(key).await,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("gateway error: {0}")]
    Gateway(String),
}
