//! FTS5 search over track titles, albums, and artists.

use rusqlite::Connection;

use super::tracks::{row_to_track, TrackRow};

/// Return up to `limit` tracks matching `query` (`SQLite` FTS5 MATCH syntax).
/// The result is ordered by FTS rank, then track id.
///
/// Supports field operators (`title:`, `album:`, `artist:`), phrase quotes,
/// `OR` / `AND`, negation (`-foo`), prefix (`*`) — see `SQLite` FTS5 docs.
///
/// ponytail: callers expect live-as-you-type behaviour, so we rewrite the
/// last whitespace-separated word into a prefix query (`foo*`) when it
/// looks like an unfinished token. Phrases (`"..."`), field operators
/// (`title:foo`), negation (`-foo`), and already-prefixed words (`foo*`)
/// are passed through unchanged.
pub fn search_tracks(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> Result<Vec<TrackRow>, rusqlite::Error> {
    let query = rewrite_as_prefix(query);
    let mut stmt = conn.prepare(
        "SELECT t.id, t.path, t.title, t.track_no, t.disc_no, t.duration_ms, t.codec, \
                t.genre, a.year, \
                a.id, a.title, ar.id, ar.name \
         FROM track_fts f \
         JOIN track t ON t.id = f.rowid \
         LEFT JOIN album a  ON a.id  = t.album_id \
         LEFT JOIN artist ar ON ar.id = a.album_artist_id \
         WHERE track_fts MATCH ?1 \
         ORDER BY rank, t.id \
         LIMIT ?2",
    )?;
    let rows: Vec<TrackRow> = stmt
        .query_map(rusqlite::params![&query, limit], row_to_track)?
        .collect::<Result<Vec<_>, _>>()?;

    // FTS5's default prefix index skips 1-char tokens — `q` matches nothing
    // even though there are tracks starting with `Q`. Fall back to a
    // case-insensitive `LIKE` over the searchable columns so single-letter
    // typing still gives feedback. We only fall back when the user gave a
    // short simple token; structured queries pass through.
    let rows = if rows.is_empty() && is_short_simple_token(&query) {
        let pat = format!("%{query}%");
        let mut stmt = conn.prepare(
            "SELECT t.id, t.path, t.title, t.track_no, t.disc_no, t.duration_ms, t.codec, \
                    t.genre, a.year, \
                    a.id, a.title, ar.id, ar.name \
             FROM track t \
             LEFT JOIN album a  ON a.id  = t.album_id \
             LEFT JOIN artist ar ON ar.id = a.album_artist_id \
             WHERE t.title  LIKE ?1 COLLATE NOCASE \
                OR a.title  LIKE ?1 COLLATE NOCASE \
                OR ar.name  LIKE ?1 COLLATE NOCASE \
             ORDER BY t.id \
             LIMIT ?2",
        )?;
        let fallback: Vec<TrackRow> = stmt
            .query_map(rusqlite::params![&pat, limit], row_to_track)?
            .collect::<Result<Vec<_>, _>>()?;
        fallback
    } else {
        rows
    };

    Ok(rows)
}

/// True when `query` is a single short alnum token (e.g. `q`, `qu`). Used to
/// decide whether to fall back to a LIKE scan when FTS returns nothing —
/// FTS5's default prefix index needs ≥2 chars.
fn is_short_simple_token(query: &str) -> bool {
    let trimmed = query.trim();
    !trimmed.is_empty() && trimmed.len() <= 2 && trimmed.chars().all(char::is_alphanumeric)
}

/// Turn `que` → `que*` so live-search feels responsive. Multi-token queries
/// only get the rewrite on the last word.
///
/// Rules — the last token is rewritten iff:
///   * it is non-empty,
///   * does not start with `-` (negation),
///   * does not contain `:` (field operator),
///   * does not end with `*` (already a prefix query),
///   * does not follow an unmatched opening `"` (we are inside a phrase),
///   * has at least one alphanumeric character.
fn rewrite_as_prefix(query: &str) -> String {
    if query.is_empty() {
        return query.to_string();
    }

    // Detect if we're inside an open phrase — count unescaped `"` so far.
    let mut in_phrase = false;
    let mut chars = 0usize;
    for c in query.chars() {
        if c == '"' {
            in_phrase = !in_phrase;
        }
        chars += 1;
        if chars > 4096 {
            break;
        }
    }
    if in_phrase {
        return query.to_string();
    }

    // Find the last whitespace-separated token.
    let last_start = query
        .rfind(|c: char| c.is_whitespace())
        .map_or(0, |i| i + 1);
    let last_token = &query[last_start..];
    if last_token.is_empty() {
        return query.to_string();
    }

    // Skip if the last token looks like a structured query fragment.
    if last_token.starts_with('-')
        || last_token.starts_with('"')
        || last_token.ends_with('*')
        || last_token.ends_with('"')
        || last_token.contains(':')
    {
        return query.to_string();
    }

    // Skip if the token is just punctuation.
    if !last_token.chars().any(char::is_alphanumeric) {
        return query.to_string();
    }

    let mut out = String::with_capacity(query.len() + 1);
    out.push_str(&query[..last_start]);
    out.push_str(last_token);
    out.push('*');
    out
}

#[cfg(test)]
mod tests {
    use super::rewrite_as_prefix;

    #[test]
    fn prefix_simple() {
        assert_eq!(rewrite_as_prefix("que"), "que*");
        assert_eq!(rewrite_as_prefix("queen"), "queen*");
    }

    #[test]
    fn prefix_already_has_star() {
        assert_eq!(rewrite_as_prefix("que*"), "que*");
    }

    #[test]
    fn prefix_phrase_left_alone() {
        // Open phrase: pass through (we don't know if the user is still
        // typing the closing quote).
        assert_eq!(rewrite_as_prefix(r#""don stop"#), r#""don stop"#);
        // Closed phrase — the last token ends with `"`, no rewrite.
        assert_eq!(rewrite_as_prefix(r#""don stop""#), r#""don stop""#);
    }

    #[test]
    fn prefix_negation_left_alone() {
        assert_eq!(rewrite_as_prefix("foo -bar"), "foo -bar");
    }

    #[test]
    fn prefix_field_left_alone() {
        assert_eq!(rewrite_as_prefix("title:foo"), "title:foo");
    }

    #[test]
    fn prefix_empty_left_alone() {
        assert_eq!(rewrite_as_prefix(""), "");
        assert_eq!(rewrite_as_prefix("   "), "   ");
    }

    #[test]
    fn prefix_multi_word() {
        assert_eq!(rewrite_as_prefix("queen dont stop"), "queen dont stop*");
        assert_eq!(rewrite_as_prefix("foo bar baz"), "foo bar baz*");
    }

    #[test]
    fn prefix_no_op_for_punctuation() {
        assert_eq!(rewrite_as_prefix("foo -"), "foo -");
    }
}
