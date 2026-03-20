mod common;

use burst_server::db;
use burst_server::storage::{Storage, local::LocalStorage};
use bytes::Bytes;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Helper: create a message, attach a file, soft-delete the message, then run cleanup.
#[sqlx::test(migrations = "../../migrations")]
async fn cleanup_removes_orphaned_attachments(pool: sqlx::PgPool) {
    // Setup: storage in a temp dir.
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::Local(LocalStorage::new(dir.path().to_path_buf()).unwrap());

    // Seed a user, channel, and message.
    let user = common::seed_user(&pool, "cleaner").await;
    let channel = common::seed_channel(&pool, "cleanup-test", user.id).await;
    let msg_id = burst_core::id::new_id();
    db::messages::create(&pool, msg_id, channel.id, user.id, None, "file msg")
        .await
        .unwrap();

    // Attach a file.
    let att_id = burst_core::id::new_id();
    let storage_key = format!("{}/att/{}", channel.id, att_id);
    storage
        .put(&storage_key, Bytes::from("file-data"), "text/plain")
        .await
        .unwrap();
    db::attachments::create(
        &pool,
        &db::attachments::CreateAttachment {
            id: att_id,
            message_id: msg_id,
            file_name: "test.txt".into(),
            file_size: 9,
            content_type: "text/plain".into(),
            storage_key: storage_key.clone(),
            metadata: serde_json::json!({}),
        },
    )
    .await
    .unwrap();

    // Verify file exists.
    assert!(storage.get(&storage_key).await.is_ok());

    // Soft-delete the message.
    db::messages::soft_delete(&pool, msg_id, user.id)
        .await
        .unwrap();

    // Backdate the deleted_at so it's older than retention.
    sqlx::query("UPDATE messages SET deleted_at = now() - interval '31 days' WHERE id = $1")
        .bind(msg_id)
        .execute(&pool)
        .await
        .unwrap();

    // Run cleanup (retention = 30 days, so 31-day-old deletion qualifies).
    let shutdown = CancellationToken::new();
    let handle = burst_server::services::cleanup::spawn(
        pool.clone(),
        storage.clone(),
        Duration::from_millis(100), // short interval for testing
        30,
        shutdown.clone(),
    );

    // Wait for one cycle to run.
    tokio::time::sleep(Duration::from_millis(500)).await;
    shutdown.cancel();
    let _ = handle.await;

    // Verify: storage file should be gone.
    assert!(storage.get(&storage_key).await.is_err());

    // Verify: attachment row should be gone.
    let att = db::attachments::find_by_id(&pool, att_id).await.unwrap();
    assert!(att.is_none());
}

/// Attachments for messages deleted less than retention_days ago should NOT be cleaned.
#[sqlx::test(migrations = "../../migrations")]
async fn cleanup_skips_recent_deletions(pool: sqlx::PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::Local(LocalStorage::new(dir.path().to_path_buf()).unwrap());

    let user = common::seed_user(&pool, "recent").await;
    let channel = common::seed_channel(&pool, "recent-test", user.id).await;
    let msg_id = burst_core::id::new_id();
    db::messages::create(&pool, msg_id, channel.id, user.id, None, "recent")
        .await
        .unwrap();

    let att_id = burst_core::id::new_id();
    let key = format!("{}/att/{}", channel.id, att_id);
    storage
        .put(&key, Bytes::from("keep-me"), "text/plain")
        .await
        .unwrap();
    db::attachments::create(
        &pool,
        &db::attachments::CreateAttachment {
            id: att_id,
            message_id: msg_id,
            file_name: "keep.txt".into(),
            file_size: 7,
            content_type: "text/plain".into(),
            storage_key: key.clone(),
            metadata: serde_json::json!({}),
        },
    )
    .await
    .unwrap();

    // Soft-delete the message (just now — within retention window).
    db::messages::soft_delete(&pool, msg_id, user.id)
        .await
        .unwrap();

    // Run cleanup with 30-day retention.
    let shutdown = CancellationToken::new();
    let handle = burst_server::services::cleanup::spawn(
        pool.clone(),
        storage.clone(),
        Duration::from_millis(100),
        30,
        shutdown.clone(),
    );

    tokio::time::sleep(Duration::from_millis(500)).await;
    shutdown.cancel();
    let _ = handle.await;

    // File and row should still exist (deleted < 30 days ago).
    assert!(storage.get(&key).await.is_ok());
    assert!(
        db::attachments::find_by_id(&pool, att_id)
            .await
            .unwrap()
            .is_some()
    );
}
