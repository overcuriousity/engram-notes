use super::Index;
use crate::Result;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FtsHit {
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub heading: Option<String>,
    /// 1-based line in the file; a text match is not tied to one passage.
    pub line: u32,
    pub score: f64,
}

/// Each word becomes a quoted prefix term, so FTS5 syntax in user input is inert.
pub fn fts_query(user: &str) -> Option<String> {
    let terms: Vec<String> = user
        .split_whitespace()
        .map(|w| format!("\"{}\"*", w.replace('"', "\"\"")))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

impl Index {
    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<FtsHit>> {
        let Some(q) = fts_query(query) else {
            return Ok(vec![]);
        };
        // Control characters mark the matches; they survive escaping and are
        // swapped for tags afterwards. Title weight 10 puts title hits first.
        let sql = "SELECT n.path, n.title,
                          snippet(notes_fts, 1, char(1), char(2), '…', 24),
                          bm25(notes_fts, 10.0, 1.0), n.body_line
                   FROM notes_fts JOIN notes n ON n.rowid = notes_fts.rowid
                   WHERE notes_fts MATCH ?1
                   ORDER BY bm25(notes_fts, 10.0, 1.0)
                   LIMIT ?2";
        Ok(self
            .conn
            .prepare(sql)?
            .query_map(rusqlite::params![q, limit as i64], |r| {
                let raw: String = r.get(2)?;
                let snippet = escape_html(&raw)
                    .replace('\u{1}', "<mark>")
                    .replace('\u{2}', "</mark>");
                Ok(FtsHit {
                    path: r.get(0)?,
                    title: r.get(1)?,
                    snippet,
                    heading: None,
                    line: r.get::<_, i64>(4)? as u32,
                    score: -r.get::<_, f64>(3)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use crate::index::Index;
    use crate::vault::Vault;
    use std::fs;

    fn ix() -> (tempfile::TempDir, Index) {
        let d = tempfile::tempdir().unwrap();
        fs::write(
            d.path().join("Rust.md"),
            "Rust ownership rules keep memory safe. <b>bold</b>",
        )
        .unwrap();
        fs::write(d.path().join("Go.md"), "Garbage collection in Go").unwrap();
        fs::write(d.path().join("Umlaut.md"), "Übung macht den Meister").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        (d, ix)
    }

    #[test]
    fn prefix_match_with_snippet() {
        let (_d, ix) = ix();
        let hits = ix.search_fts("owner", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "Rust.md");
        assert!(hits[0].snippet.contains("<mark>ownership</mark>"));
        assert!(hits[0].snippet.contains("&lt;b&gt;"));
    }

    #[test]
    fn title_matches_rank_first_and_syntax_is_inert() {
        let (_d, ix) = ix();
        let hits = ix.search_fts("go", 10).unwrap();
        assert_eq!(hits[0].path, "Go.md");
        assert!(ix.search_fts("\"(", 10).is_ok());
        assert!(ix.search_fts("   ", 10).unwrap().is_empty());
    }

    #[test]
    fn diacritics_are_folded() {
        let (_d, ix) = ix();
        assert_eq!(ix.search_fts("ubung", 10).unwrap()[0].path, "Umlaut.md");
    }
}
