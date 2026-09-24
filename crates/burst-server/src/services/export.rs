//! Data export: every message and file in scope, as one zip archive.
//!
//! Layout:
//!
//! ```text
//! export.json                      what was exported, when, by whom
//! users.json                       the users the export names
//! channels/<channel id>/channel.json   the channel and its members
//! channels/<channel id>/messages.jsonl one message per line, oldest first
//! files/<attachment id>/<file name>    attachments, as uploaded
//! ```
//!
//! Deleted messages and their files are left out. The archive is written to a
//! temporary file on a blocking thread, then streamed to storage, so neither
//! the archive nor the whole of any channel is held in memory.

use std::collections::HashMap;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::runtime::Handle;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::AppState;
use crate::db;
use crate::db::exports::ExportRow;
use burst_core::id::{
    format_attachment_id, format_channel_id, format_export_id, format_message_id, format_user_id,
};

/// The archive format, for anyone reading it back.
pub const FORMAT: &str = "burst-export/1";
/// Messages read from the database at a time.
const PAGE: i64 = 500;
/// A running export not touched for this long is treated as dead.
pub const STALL_AFTER: chrono::Duration = chrono::Duration::minutes(10);

pub fn storage_key(id: Uuid) -> String {
    format!("exports/{id}.zip")
}

/// Starts building `export` in the background.
pub fn spawn(state: AppState, export: ExportRow) {
    tokio::spawn(async move {
        let id = export.id;
        if let Err(e) = run(&state, export).await {
            tracing::error!(export_id = %id, error = %e, "export failed");
            if let Err(e) = db::exports::fail(&state.db, id, &e.to_string()).await {
                tracing::error!(export_id = %id, error = %e, "could not record export failure");
            }
        }
    });
}

#[derive(Debug, thiserror::Error)]
enum ExportError {
    #[error("database: {0}")]
    Db(#[from] sqlx::Error),
    #[error("storage: {0}")]
    Storage(#[from] crate::storage::StorageError),
    #[error("archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("worker: {0}")]
    Join(#[from] tokio::task::JoinError),
}

async fn run(state: &AppState, export: ExportRow) -> Result<(), ExportError> {
    db::exports::touch_running(&state.db, export.id).await?;
    let path = temp_path(export.id);

    let built = {
        let (state, export, path, handle) = (
            state.clone(),
            export.clone(),
            path.clone(),
            Handle::current(),
        );
        tokio::task::spawn_blocking(move || build(&handle, &state, &export, &path)).await?
    };
    let uploaded = match built {
        Ok(()) => upload(state, export.id, &path).await,
        Err(e) => Err(e),
    };
    let _ = tokio::fs::remove_file(&path).await;
    uploaded
}

async fn upload(state: &AppState, id: Uuid, path: &Path) -> Result<(), ExportError> {
    let size = tokio::fs::metadata(path).await?.len();
    let key = storage_key(id);
    state
        .storage
        .put_file(&key, path, "application/zip")
        .await?;
    db::exports::complete(&state.db, id, &key, i64::try_from(size).unwrap_or(i64::MAX)).await?;
    tracing::info!(export_id = %id, bytes = size, "export completed");
    Ok(())
}

// ── Archive ──────────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    format: &'static str,
    export_id: String,
    scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_id: Option<String>,
    requested_by: String,
    created_at: DateTime<Utc>,
    channels: usize,
    messages: u64,
    files: u64,
    /// Attachments recorded in the database whose file was not in storage.
    missing_files: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UserEntry {
    id: String,
    username: String,
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    role: String,
    is_bot: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    deactivated_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelEntry {
    id: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    created_by: String,
    is_archived: bool,
    created_at: DateTime<Utc>,
    members: Vec<MemberEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemberEntry {
    user_id: String,
    role: String,
    joined_at: DateTime<Utc>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MessageEntry {
    id: String,
    user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_id: Option<String>,
    content: String,
    created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edited_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attachments: Vec<AttachmentEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    reactions: Vec<ReactionEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentEntry {
    id: String,
    file_name: String,
    size: i64,
    content_type: String,
    /// Where the file is in the archive; absent when it was missing from storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReactionEntry {
    emoji: String,
    user_ids: Vec<String>,
}

/// Runs on a blocking thread; `handle` runs the database and storage calls.
fn build(
    handle: &Handle,
    state: &AppState,
    export: &ExportRow,
    path: &Path,
) -> Result<(), ExportError> {
    let file = std::fs::File::create(path)?;
    let mut zip = ZipWriter::new(BufWriter::new(file));
    let json = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    let users = handle.block_on(db::exports::users_in_scope(&state.db, export.channel_id))?;
    let users: Vec<UserEntry> = users
        .into_iter()
        .map(|u| UserEntry {
            id: format_user_id(u.id),
            username: u.username,
            display_name: u.display_name,
            email: u.email,
            role: u.role,
            is_bot: u.is_bot,
            deactivated_at: u.deactivated_at,
            created_at: u.created_at,
        })
        .collect();
    zip.start_file("users.json", json)?;
    serde_json::to_writer_pretty(&mut zip, &users)?;

    let channels = handle.block_on(db::exports::channels_in_scope(&state.db, export.channel_id))?;
    let (mut messages, mut files, mut missing) = (0u64, 0u64, Vec::new());

    for channel in &channels {
        handle.block_on(db::exports::touch_running(&state.db, export.id))?;
        let dir = format!("channels/{}", format_channel_id(channel.id));
        let members = handle.block_on(db::channels::list_members(&state.db, channel.id))?;
        zip.start_file(format!("{dir}/channel.json"), json)?;
        serde_json::to_writer_pretty(
            &mut zip,
            &ChannelEntry {
                id: format_channel_id(channel.id),
                kind: channel.kind.clone(),
                name: channel.name.clone(),
                topic: channel.topic.clone(),
                description: channel.description.clone(),
                created_by: format_user_id(channel.created_by),
                is_archived: channel.is_archived,
                created_at: channel.created_at,
                members: members
                    .into_iter()
                    .map(|m| MemberEntry {
                        user_id: format_user_id(m.user_id),
                        role: m.role,
                        joined_at: m.joined_at,
                    })
                    .collect(),
            },
        )?;

        // A zip entry must be complete before the next starts, and files are
        // entries of their own, so the lines go to a side file first.
        let lines_path = temp_path(export.id).with_extension(format!("{}.jsonl", channel.id));
        let mut lines = BufWriter::new(std::fs::File::create(&lines_path)?);
        let mut after = None;
        loop {
            let page = handle.block_on(db::exports::messages_page(
                &state.db, channel.id, after, PAGE,
            ))?;
            let Some(last) = page.last() else { break };
            after = Some(last.id);
            let ids: Vec<Uuid> = page.iter().map(|m| m.id).collect();
            let mut attachments = group(
                handle.block_on(db::attachments::list_for_messages(&state.db, &ids))?,
                |a| a.message_id,
            );
            let mut reactions = group(
                handle.block_on(db::reactions::list_for_messages(&state.db, &ids))?,
                |r| r.message_id,
            );

            for m in page {
                let mut attachment_entries = Vec::new();
                for a in attachments.remove(&m.id).unwrap_or_default() {
                    let archived = format!(
                        "files/{}/{}",
                        format_attachment_id(a.id),
                        archive_file_name(&a.file_name)
                    );
                    let path = match handle.block_on(state.storage.get(&a.storage_key)) {
                        Ok((data, _)) => {
                            zip.start_file(archived.as_str(), stored)?;
                            zip.write_all(&data)?;
                            files += 1;
                            Some(archived)
                        }
                        Err(crate::storage::StorageError::NotFound(_)) => {
                            missing.push(format_attachment_id(a.id));
                            None
                        }
                        Err(e) => return Err(e.into()),
                    };
                    attachment_entries.push(AttachmentEntry {
                        id: format_attachment_id(a.id),
                        file_name: a.file_name,
                        size: a.file_size,
                        content_type: a.content_type,
                        path,
                    });
                }
                let entry = MessageEntry {
                    id: format_message_id(m.id),
                    user_id: format_user_id(m.user_id),
                    thread_id: m.thread_id.map(format_message_id),
                    content: m.content,
                    created_at: m.created_at,
                    edited_at: m.edited_at,
                    attachments: attachment_entries,
                    reactions: reaction_entries(reactions.remove(&m.id).unwrap_or_default()),
                };
                serde_json::to_writer(&mut lines, &entry)?;
                lines.write_all(b"\n")?;
                messages += 1;
            }
            handle.block_on(db::exports::touch_running(&state.db, export.id))?;
        }
        lines.flush()?;
        drop(lines);

        zip.start_file(format!("{dir}/messages.jsonl"), json.large_file(true))?;
        std::io::copy(&mut std::fs::File::open(&lines_path)?, &mut zip)?;
        std::fs::remove_file(&lines_path)?;
    }

    zip.start_file("export.json", json)?;
    serde_json::to_writer_pretty(
        &mut zip,
        &Manifest {
            format: FORMAT,
            export_id: format_export_id(export.id),
            scope: export.scope.clone(),
            channel_id: export.channel_id.map(format_channel_id),
            requested_by: format_user_id(export.requested_by),
            created_at: Utc::now(),
            channels: channels.len(),
            messages,
            files,
            missing_files: missing,
        },
    )?;
    zip.finish()?.flush()?;
    Ok(())
}

fn group<T>(rows: Vec<T>, key: impl Fn(&T) -> Uuid) -> HashMap<Uuid, Vec<T>> {
    let mut map: HashMap<Uuid, Vec<T>> = HashMap::new();
    for row in rows {
        map.entry(key(&row)).or_default().push(row);
    }
    map
}

fn reaction_entries(rows: Vec<db::reactions::ReactionRow>) -> Vec<ReactionEntry> {
    let mut entries: Vec<ReactionEntry> = Vec::new();
    for r in rows {
        let user = format_user_id(r.user_id);
        match entries.iter_mut().find(|e| e.emoji == r.emoji) {
            Some(e) => e.user_ids.push(user),
            None => entries.push(ReactionEntry {
                emoji: r.emoji,
                user_ids: vec![user],
            }),
        }
    }
    entries
}

/// A file name safe to extract: no directories, no leading dots, never empty.
fn archive_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim_start_matches('.').trim();
    if cleaned.is_empty() {
        "file".to_string()
    } else {
        cleaned.to_string()
    }
}

/// The temporary file an export is built in.
pub fn temp_path(id: Uuid) -> PathBuf {
    std::env::temp_dir().join(format!("burst-export-{id}.zip"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_cannot_escape_their_directory() {
        assert_eq!(archive_file_name("report.pdf"), "report.pdf");
        assert_eq!(archive_file_name("../../etc/passwd"), "passwd");
        assert_eq!(archive_file_name("C:\\\\Users\\\\x\\\\a.txt"), "a.txt");
        assert_eq!(archive_file_name(".."), "file");
        assert_eq!(archive_file_name(".hidden"), "hidden");
        assert_eq!(archive_file_name("a:b?.txt"), "a_b_.txt");
        assert_eq!(archive_file_name(""), "file");
    }

    #[test]
    fn reactions_are_grouped_by_emoji_in_first_seen_order() {
        let row = |user: u128, emoji: &str| db::reactions::ReactionRow {
            message_id: Uuid::nil(),
            user_id: Uuid::from_u128(user),
            emoji: emoji.into(),
            created_at: Utc::now(),
        };
        let entries = reaction_entries(vec![row(1, "👍"), row(2, "🎉"), row(3, "👍")]);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].emoji, "👍");
        assert_eq!(entries[0].user_ids.len(), 2);
        assert_eq!(entries[1].emoji, "🎉");
    }
}
