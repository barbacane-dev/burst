use bytes::Bytes;

use super::StorageError;

/// S3 storage via Barbacane gateway.
///
/// Proxies PUT/GET/DELETE operations to the Barbacane gateway which routes
/// them through its S3 dispatcher plugin. Burst never touches AWS credentials
/// directly — the gateway handles SigV4 signing (ADR-011).
#[derive(Debug, Clone)]
pub struct GatewayStorage {
    client: reqwest::Client,
    base_url: String,
}

impl GatewayStorage {
    pub fn new(gateway_url: &str) -> Self {
        let base_url = gateway_url.trim_end_matches('/').to_string();
        Self {
            client: reqwest::Client::new(),
            base_url,
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
        let resp = self
            .client
            .put(self.url(key))
            .header("content-type", content_type)
            .body(data)
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
        let resp = self
            .client
            .get(self.url(key))
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

    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        let resp = self
            .client
            .delete(self.url(key))
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
        let storage = GatewayStorage::new("http://localhost:8080");
        assert_eq!(
            storage.url("ch_123/2026/03/att_456/file.pdf"),
            "http://localhost:8080/storage/ch_123/2026/03/att_456/file.pdf"
        );
    }

    #[test]
    fn url_strips_trailing_slash() {
        let storage = GatewayStorage::new("http://localhost:8080/");
        assert_eq!(
            storage.url("test.txt"),
            "http://localhost:8080/storage/test.txt"
        );
    }
}
