use std::path::Path;

use bytes::Bytes;
use futures_util::TryStreamExt;

use super::StorageError;

/// S3 storage via Barbacane S3 sidecar.
///
/// Proxies PUT/GET/DELETE operations to a dedicated Barbacane instance running
/// only the S3 dispatcher plugin (burst-s3.bca). Burst never touches AWS
/// credentials — the sidecar handles SigV4 signing (ADR-011).
///
/// Authenticated via API key (X-Storage-Key header) for defense-in-depth.
#[derive(Debug, Clone)]
pub struct GatewayStorage {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl GatewayStorage {
    pub fn new(gateway_url: &str, api_key: Option<String>) -> Self {
        let base_url = gateway_url.trim_end_matches('/').to_string();
        Self {
            client: reqwest::Client::new(),
            base_url,
            api_key,
        }
    }

    fn with_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(key) => builder.header("X-Storage-Key", key),
            None => builder,
        }
    }

    fn url(&self, key: &str) -> String {
        format!("{}/storage/{}", self.base_url, key)
    }

    pub async fn put(
        &self,
        key: &str,
        data: Bytes,
        content_type: &str,
    ) -> Result<(), StorageError> {
        let req = self
            .client
            .put(self.url(key))
            .header("content-type", content_type)
            .body(data);
        let resp = self
            .with_auth(req)
            .send()
            .await
            .map_err(|e| StorageError::Gateway(e.to_string()))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(StorageError::Gateway(format!(
                "PUT {} returned {}",
                key,
                resp.status()
            )))
        }
    }

    pub async fn get(&self, key: &str) -> Result<(Bytes, String), StorageError> {
        let req = self.client.get(self.url(key));
        let resp = self
            .with_auth(req)
            .send()
            .await
            .map_err(|e| StorageError::Gateway(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(StorageError::NotFound(key.to_string()));
        }

        if !resp.status().is_success() {
            return Err(StorageError::Gateway(format!(
                "GET {} returned {}",
                key,
                resp.status()
            )));
        }

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        let data = resp
            .bytes()
            .await
            .map_err(|e| StorageError::Gateway(e.to_string()))?;

        Ok((data, content_type))
    }

    /// Uploads a file as a single streamed PUT.
    pub async fn put_file(
        &self,
        key: &str,
        path: &Path,
        content_type: &str,
    ) -> Result<(), StorageError> {
        let file = tokio::fs::File::open(path).await?;
        let len = file.metadata().await?.len();
        let body = reqwest::Body::wrap_stream(tokio_util::io::ReaderStream::new(file));
        let req = self
            .client
            .put(self.url(key))
            .header("content-type", content_type)
            .header("content-length", len)
            .body(body);
        let resp = self
            .with_auth(req)
            .send()
            .await
            .map_err(|e| StorageError::Gateway(e.to_string()))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(StorageError::Gateway(format!(
                "PUT {} returned {}",
                key,
                resp.status()
            )))
        }
    }

    pub async fn get_stream(
        &self,
        key: &str,
    ) -> Result<(super::ByteStream, Option<u64>), StorageError> {
        let req = self.client.get(self.url(key));
        let resp = self
            .with_auth(req)
            .send()
            .await
            .map_err(|e| StorageError::Gateway(e.to_string()))?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(StorageError::NotFound(key.to_string()));
        }
        if !resp.status().is_success() {
            return Err(StorageError::Gateway(format!(
                "GET {} returned {}",
                key,
                resp.status()
            )));
        }
        let len = resp.content_length();
        let stream = resp.bytes_stream().map_err(std::io::Error::other);
        Ok((Box::pin(stream), len))
    }

    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        let req = self.client.delete(self.url(key));
        let resp = self
            .with_auth(req)
            .send()
            .await
            .map_err(|e| StorageError::Gateway(e.to_string()))?;

        if resp.status().is_success() || resp.status() == reqwest::StatusCode::NOT_FOUND {
            Ok(())
        } else {
            Err(StorageError::Gateway(format!(
                "DELETE {} returned {}",
                key,
                resp.status()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_construction() {
        let storage = GatewayStorage::new("http://localhost:8080", None);
        assert_eq!(
            storage.url("ch_123/2026/03/att_456/file.pdf"),
            "http://localhost:8080/storage/ch_123/2026/03/att_456/file.pdf"
        );
    }

    #[test]
    fn url_strips_trailing_slash() {
        let storage = GatewayStorage::new("http://localhost:8080/", None);
        assert_eq!(
            storage.url("test.txt"),
            "http://localhost:8080/storage/test.txt"
        );
    }
}
