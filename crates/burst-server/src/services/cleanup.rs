use std::time::Duration;

use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::storage::Storage;

/// Maximum number of attachments to clean up per cycle (avoids long transactions).
const BATCH_SIZE: i64 = 100;

/// Row returned by the orphan query.
#[derive(sqlx::FromRow)]
struct OrphanedAttachment {
    id: Uuid,
    storage_key: String,
}

/// Spawns a background task that periodically removes files for soft-deleted
/// messages whose `deleted_at` is older than `retention_days`.
///
/// The task runs every `interval` and processes up to [`BATCH_SIZE`] attachments
/// per cycle. Errors are logged but do not crash the task.
pub fn spawn(
    pool: PgPool,
    storage: Storage,
    interval: Duration,
    retention_days: i64,
    shutdown: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!(
            interval_secs = interval.as_secs(),
            retention_days,
            "attachment cleanup task started"
        );

        loop {
            tokio::select! {
                () = tokio::time::sleep(interval) => {}
                () = shutdown.cancelled() => {
                    tracing::info!("attachment cleanup task stopping");
                    return;
                }
            }

            match cleanup_cycle(&pool, &storage, retention_days).await {
                Ok(count) if count > 0 => {
                    tracing::info!(removed = count, "cleaned up orphaned attachments");
                }
                Ok(_) => {} // nothing to clean
                Err(e) => {
                    tracing::warn!(error = %e, "attachment cleanup cycle failed");
                }
            }
        }
    })
}

/// Run one cleanup cycle: find orphaned attachments, delete their storage
/// objects, then delete the DB rows. Returns the number removed.
async fn cleanup_cycle(
    pool: &PgPool,
    storage: &Storage,
    retention_days: i64,
) -> Result<usize, sqlx::Error> {
    let orphans = sqlx::query_as::<_, OrphanedAttachment>(
        "SELECT a.id, a.storage_key \
         FROM attachments a \
         JOIN messages m ON a.message_id = m.id \
         WHERE m.deleted_at IS NOT NULL \
           AND m.deleted_at < now() - ($1 || ' days')::interval \
         LIMIT $2",
    )
    .bind(retention_days.to_string())
    .bind(BATCH_SIZE)
    .fetch_all(pool)
    .await?;

    if orphans.is_empty() {
        return Ok(0);
    }

    let mut removed = 0;
    for orphan in &orphans {
        // Delete from storage first (idempotent).
        if let Err(e) = storage.delete(&orphan.storage_key).await {
            tracing::warn!(
                attachment_id = %orphan.id,
                storage_key = %orphan.storage_key,
                error = %e,
                "failed to delete storage object, skipping"
            );
            continue;
        }

        // Delete the DB row.
        sqlx::query("DELETE FROM attachments WHERE id = $1")
            .bind(orphan.id)
            .execute(pool)
            .await?;

        removed += 1;
    }

    Ok(removed)
}
