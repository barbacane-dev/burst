use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct SearchResultRow {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub user_id: Uuid,
    pub thread_id: Option<Uuid>,
    pub content: String,
    pub headline: String,
    pub created_at: DateTime<Utc>,
    /// The attachment name that matched, when a file name matched the query.
    pub matched_file: Option<String>,
    /// Relevance, for the next page's cursor.
    pub rank: f32,
}

/// Where the previous page ended: results are ordered by relevance, then id.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SearchCursor {
    pub rank: f32,
    pub id: Uuid,
}

/// What narrows a search, beyond the query itself.
#[derive(Debug, Default, Clone)]
pub struct SearchFilters {
    pub channel_id: Option<Uuid>,
    /// Only messages this user wrote.
    pub from: Option<Uuid>,
    /// Only messages sent at or after this instant.
    pub after: Option<DateTime<Utc>>,
    /// Only messages sent before this instant.
    pub before: Option<DateTime<Utc>>,
    /// Only messages carrying at least one file.
    pub has_file: bool,
}

/// The rank a match on a file name alone is given. Above zero, so a message
/// found only by its attachment is not placed after every content match
/// regardless, and below what a content match normally scores.
const FILE_NAME_RANK: f32 = 0.05;

/// Full-text search across messages the user has access to, and across the
/// names of the files attached to them.
///
/// Uses PostgreSQL `websearch_to_tsquery` for user-friendly query syntax
/// (quoted phrases, `-exclude`) and `ts_headline` for highlighted snippets.
/// File names match by case-insensitive substring: names like `Q3-report.pdf`
/// do not tokenise into the words people search for. Access control is
/// enforced by joining `channel_members`.
///
/// Results are ordered by relevance and paged by (relevance, id), so a page
/// continues exactly where the previous one ended.
pub async fn search_messages(
    pool: &PgPool,
    user_id: Uuid,
    query: &str,
    filters: &SearchFilters,
    cursor: Option<SearchCursor>,
    limit: i64,
) -> Result<Vec<SearchResultRow>, sqlx::Error> {
    sqlx::query_as::<_, SearchResultRow>(
        "WITH q AS (SELECT websearch_to_tsquery('english', $1) AS tsq) \
         SELECT * FROM ( \
           SELECT m.id, m.channel_id, m.user_id, m.thread_id, m.content, \
             ts_headline('english', m.content, q.tsq, \
               'StartSel=<mark>, StopSel=</mark>, MaxFragments=2, MaxWords=30') AS headline, \
             m.created_at, \
             f.file_name AS matched_file, \
             GREATEST(ts_rank(m.search_vec, q.tsq), \
               CASE WHEN f.file_name IS NOT NULL THEN $4::real ELSE 0 END)::real AS rank \
           FROM messages m \
           CROSS JOIN q \
           JOIN channel_members cm ON cm.channel_id = m.channel_id AND cm.user_id = $2 \
           LEFT JOIN LATERAL ( \
             SELECT a.file_name FROM attachments a \
             WHERE a.message_id = m.id AND a.file_name ILIKE $3 ESCAPE '\\' \
             ORDER BY a.created_at, a.id LIMIT 1 \
           ) f ON true \
           WHERE (m.search_vec @@ q.tsq OR f.file_name IS NOT NULL) \
             AND m.deleted_at IS NULL \
             AND ($5::uuid IS NULL OR m.channel_id = $5) \
             AND ($6::uuid IS NULL OR m.user_id = $6) \
             AND ($7::timestamptz IS NULL OR m.created_at >= $7) \
             AND ($8::timestamptz IS NULL OR m.created_at < $8) \
             AND (NOT $9 OR EXISTS (SELECT 1 FROM attachments a2 WHERE a2.message_id = m.id)) \
         ) r \
         WHERE ($10::real IS NULL OR (r.rank, r.id) < ($10, $11)) \
         ORDER BY r.rank DESC, r.id DESC \
         LIMIT $12",
    )
    .bind(query)
    .bind(user_id)
    .bind(substring_pattern(query))
    .bind(FILE_NAME_RANK)
    .bind(filters.channel_id)
    .bind(filters.from)
    .bind(filters.after)
    .bind(filters.before)
    .bind(filters.has_file)
    .bind(cursor.map(|c| c.rank))
    .bind(cursor.map(|c| c.id))
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// An `ILIKE` pattern matching `text` anywhere, with its wildcards escaped so
/// a `%` or `_` in the query is matched literally.
fn substring_pattern(text: &str) -> String {
    let mut pattern = String::with_capacity(text.len() + 2);
    pattern.push('%');
    for c in text.chars() {
        if matches!(c, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    pattern.push('%');
    pattern
}

#[cfg(test)]
mod tests {
    use super::substring_pattern;

    #[test]
    fn wraps_the_query_to_match_anywhere() {
        assert_eq!(substring_pattern("report"), "%report%");
    }

    #[test]
    fn escapes_wildcards_so_they_match_literally() {
        assert_eq!(substring_pattern("50%_off\\x"), "%50\\%\\_off\\\\x%");
    }
}
