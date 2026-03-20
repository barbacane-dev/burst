mod common;

use axum::Router;
use axum::body::Body;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, put};
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

use burst_server::storage::StorageError;
use burst_server::storage::gateway::GatewayStorage;

// ── Mock S3 tests (always run, no external deps) ────────────────────────────

type FileStore = Arc<RwLock<HashMap<String, (Bytes, String)>>>;

/// In-memory mock of the Barbacane S3 dispatcher.
/// Stores files in a HashMap, mimicking PUT/GET/DELETE on /storage/{key}.
fn mock_s3_router() -> (Router, FileStore) {
    let store: FileStore = Arc::new(RwLock::new(HashMap::new()));

    let s = store.clone();
    let put_handler = put(
        move |Path(key): Path<String>, req: axum::extract::Request| {
            let store = s.clone();
            async move {
                let content_type = req
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let data = axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024)
                    .await
                    .unwrap();
                store.write().await.insert(key, (data, content_type));
                StatusCode::OK
            }
        },
    );

    let s = store.clone();
    let get_handler = get(move |Path(key): Path<String>| {
        let store = s.clone();
        async move {
            match store.read().await.get(&key) {
                Some((data, ct)) => (
                    StatusCode::OK,
                    [("content-type", ct.clone())],
                    Body::from(data.clone()),
                )
                    .into_response(),
                None => StatusCode::NOT_FOUND.into_response(),
            }
        }
    });

    let s = store.clone();
    let delete_handler = delete(move |Path(key): Path<String>| {
        let store = s.clone();
        async move {
            store.write().await.remove(&key);
            StatusCode::NO_CONTENT
        }
    });

    let router = Router::new().route(
        "/storage/{*key}",
        put_handler.merge(get_handler).merge(delete_handler),
    );

    (router, store)
}

async fn start_mock_server() -> (String, FileStore) {
    let (router, store) = mock_s3_router();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, router).into_future());
    (format!("http://{addr}"), store)
}

#[tokio::test]
async fn put_and_get_roundtrip() {
    let (base_url, _store) = start_mock_server().await;
    let storage = GatewayStorage::new(&base_url);

    storage
        .put("test/file.txt", Bytes::from("hello gateway"), "text/plain")
        .await
        .unwrap();

    let (data, ct) = storage.get("test/file.txt").await.unwrap();
    assert_eq!(data, Bytes::from("hello gateway"));
    assert_eq!(ct, "text/plain");
}

#[tokio::test]
async fn get_nonexistent_returns_not_found() {
    let (base_url, _store) = start_mock_server().await;
    let storage = GatewayStorage::new(&base_url);

    match storage.get("does/not/exist.txt").await {
        Err(StorageError::NotFound(_)) => {} // expected
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[tokio::test]
async fn delete_removes_object() {
    let (base_url, _store) = start_mock_server().await;
    let storage = GatewayStorage::new(&base_url);

    storage
        .put("delete-me.txt", Bytes::from("data"), "text/plain")
        .await
        .unwrap();

    storage.delete("delete-me.txt").await.unwrap();

    match storage.get("delete-me.txt").await {
        Err(StorageError::NotFound(_)) => {} // expected
        other => panic!("expected NotFound after delete, got {other:?}"),
    }
}

#[tokio::test]
async fn delete_nonexistent_is_idempotent() {
    let (base_url, _store) = start_mock_server().await;
    let storage = GatewayStorage::new(&base_url);

    // Deleting something that doesn't exist should not error.
    storage.delete("never-existed.txt").await.unwrap();
}

#[tokio::test]
async fn put_preserves_content_type() {
    let (base_url, store) = start_mock_server().await;
    let storage = GatewayStorage::new(&base_url);

    storage
        .put("image.png", Bytes::from("fake-png"), "image/png")
        .await
        .unwrap();

    let stored = store.read().await;
    let (_, ct) = stored.get("image.png").unwrap();
    assert_eq!(ct, "image/png");
}

#[tokio::test]
async fn put_large_file() {
    let (base_url, _store) = start_mock_server().await;
    let storage = GatewayStorage::new(&base_url);

    let big = Bytes::from(vec![0u8; 5 * 1024 * 1024]); // 5 MB
    storage
        .put("big.bin", big.clone(), "application/octet-stream")
        .await
        .unwrap();

    let (data, _) = storage.get("big.bin").await.unwrap();
    assert_eq!(data.len(), 5 * 1024 * 1024);
}

#[tokio::test]
async fn nested_key_paths() {
    let (base_url, _store) = start_mock_server().await;
    let storage = GatewayStorage::new(&base_url);

    let key = "ch_abc123/2026/03/att_def456/report.pdf";
    storage
        .put(key, Bytes::from("pdf-data"), "application/pdf")
        .await
        .unwrap();

    let (data, ct) = storage.get(key).await.unwrap();
    assert_eq!(data, Bytes::from("pdf-data"));
    assert_eq!(ct, "application/pdf");
}
