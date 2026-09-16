use super::Index;
use crate::Result;
use rusqlite::OptionalExtension;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NoteSummary {
    pub path: String,
    pub title: String,
    pub mtime_ms: i64,
    pub size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LinkRow {
    pub src_path: String,
    pub target_raw: String,
    pub target_path: Option<String>,
    pub kind: String,
    pub heading: Option<String>,
    pub block: Option<String>,
    pub alias: Option<String>,
    pub line: u32,
    pub context: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Unresolved {
    pub target: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PropertyCount {
    pub key: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}

const LINK_COLUMNS: &str = "l.src_path, l.target_raw, l.target_path, l.kind, l.heading, l.alias, l.line, n.body, n.body_line, l.block";

// The index stores the body without its frontmatter and the line the body
// starts on, so a link's source line maps onto a body line by subtraction.
fn context_line(body: &str, line: u32, body_line: i64) -> String {
    let idx = (line as i64 - body_line).max(0) as usize;
    body.lines().nth(idx).unwrap_or("").trim().to_owned()
}

fn link_row(r: &rusqlite::Row) -> rusqlite::Result<LinkRow> {
    let body: String = r.get(7)?;
    let line: u32 = r.get(6)?;
    let body_line: i64 = r.get(8)?;
    Ok(LinkRow {
        src_path: r.get(0)?,
        target_raw: r.get(1)?,
        target_path: r.get(2)?,
        kind: r.get(3)?,
        heading: r.get(4)?,
        block: r.get(9)?,
        alias: r.get(5)?,
        line,
        context: context_line(&body, line, body_line),
    })
}

fn summary(r: &rusqlite::Row) -> rusqlite::Result<NoteSummary> {
    Ok(NoteSummary {
        path: r.get(0)?,
        title: r.get(1)?,
        mtime_ms: r.get(2)?,
        size: r.get(3)?,
    })
}

impl Index {
    pub fn notes(&self) -> Result<Vec<NoteSummary>> {
        Ok(self
            .conn
            .prepare("SELECT path, title, mtime_ms, size FROM notes ORDER BY path")?
            .query_map([], summary)?
            .collect::<std::result::Result<_, _>>()?)
    }

    pub fn note(&self, path: &str) -> Result<Option<NoteSummary>> {
        Ok(self
            .conn
            .query_row(
                "SELECT path, title, mtime_ms, size FROM notes WHERE path=?1",
                [path],
                summary,
            )
            .optional()?)
    }

    pub fn backlinks(&self, path: &str) -> Result<Vec<LinkRow>> {
        let sql = format!(
            "SELECT {LINK_COLUMNS} FROM links l JOIN notes n ON n.path=l.src_path WHERE l.target_path=?1 ORDER BY l.src_path, l.line"
        );
        Ok(self
            .conn
            .prepare(&sql)?
            .query_map([path], link_row)?
            .collect::<std::result::Result<_, _>>()?)
    }

    pub fn outgoing(&self, path: &str) -> Result<Vec<LinkRow>> {
        let sql = format!(
            "SELECT {LINK_COLUMNS} FROM links l JOIN notes n ON n.path=l.src_path WHERE l.src_path=?1 ORDER BY l.start"
        );
        Ok(self
            .conn
            .prepare(&sql)?
            .query_map([path], link_row)?
            .collect::<std::result::Result<_, _>>()?)
    }

    pub fn unresolved(&self) -> Result<Vec<Unresolved>> {
        Ok(self
            .conn
            .prepare(
                "SELECT target_raw, count(*) FROM links WHERE target_path IS NULL AND kind != 'embed'
                 GROUP BY target_raw ORDER BY count(*) DESC, target_raw",
            )?
            .query_map([], |r| {
                Ok(Unresolved {
                    target: r.get(0)?,
                    count: r.get(1)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?)
    }

    pub fn tags(&self) -> Result<Vec<TagCount>> {
        Ok(self
            .conn
            .prepare("SELECT tag, count(*) FROM tags GROUP BY tag ORDER BY tag")?
            .query_map([], |r| {
                Ok(TagCount {
                    tag: r.get(0)?,
                    count: r.get(1)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?)
    }

    pub fn properties(&self, path: &str) -> Result<serde_json::Map<String, serde_json::Value>> {
        let fm: Option<String> = self
            .conn
            .query_row("SELECT frontmatter FROM notes WHERE path=?1", [path], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(fm
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default())
    }

    /// The source line a `heading` or `^block` fragment points at in `path`.
    pub fn anchor_line(&self, path: &str, fragment: &str) -> Result<Option<u32>> {
        if let Some(id) = fragment.strip_prefix('^') {
            return Ok(self
                .conn
                .query_row(
                    "SELECT line FROM blocks WHERE path=?1 AND lower(id)=lower(?2) LIMIT 1",
                    rusqlite::params![path, id],
                    |r| r.get(0),
                )
                .optional()?);
        }
        // `[[Note#A#B]]` names heading B under A; the last part finds it. The
        // match is on what a link can spell, so it happens here and not in SQL.
        let want = crate::linking::heading_key(fragment.rsplit('#').next().unwrap_or(fragment));
        let mut st = self
            .conn
            .prepare("SELECT text, line FROM headings WHERE path=?1 ORDER BY line")?;
        let mut rows = st.query(rusqlite::params![path])?;
        while let Some(r) = rows.next()? {
            if crate::linking::heading_key(&r.get::<_, String>(0)?) == want {
                return Ok(Some(r.get(1)?));
            }
        }
        Ok(None)
    }

    /// Every property key in the vault and how many notes carry it.
    pub fn property_counts(&self) -> Result<Vec<PropertyCount>> {
        Ok(self
            .conn
            .prepare(
                "SELECT key, count(*) FROM properties GROUP BY key
                 ORDER BY count(*) DESC, key",
            )?
            .query_map([], |r| {
                Ok(PropertyCount {
                    key: r.get(0)?,
                    count: r.get(1)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?)
    }

    /// The body as indexed, without the frontmatter.
    pub fn body(&self, path: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT body FROM notes WHERE path=?1", [path], |r| r.get(0))
            .optional()?)
    }

    pub fn titles(&self) -> Result<Vec<(String, String)>> {
        Ok(self
            .conn
            .prepare("SELECT path, title FROM notes ORDER BY path")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use crate::index::Index;
    use crate::vault::Vault;

    fn indexed(files: &[(&str, &str)]) -> (tempfile::TempDir, Index) {
        let d = tempfile::tempdir().unwrap();
        for (path, text) in files {
            std::fs::write(d.path().join(path), text).unwrap();
        }
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        (d, ix)
    }

    #[test]
    fn property_counts_are_ordered_by_use() {
        let (_d, ix) = indexed(&[
            ("A.md", "---\nstatus: open\ntags: [x]\n---\na"),
            ("B.md", "---\nstatus: done\n---\nb"),
            ("C.md", "no frontmatter"),
        ]);
        let counts = ix.property_counts().unwrap();
        assert_eq!(counts[0].key, "status");
        assert_eq!(counts[0].count, 2);
        assert_eq!(counts[1].key, "tags");
        assert_eq!(counts[1].count, 1);
        assert_eq!(counts.len(), 2);
    }

    #[test]
    fn anchor_line_finds_a_heading_a_link_cannot_spell() {
        let (_d, ix) = indexed(&[("A.md", "# Title\nintro\n\n## Pros | Cons\nbody\n")]);
        // What the picker writes for that heading, matched either case.
        assert_eq!(ix.anchor_line("A.md", "Pros Cons").unwrap(), Some(4));
        assert_eq!(ix.anchor_line("A.md", "pros cons").unwrap(), Some(4));
        assert_eq!(ix.anchor_line("A.md", "Title").unwrap(), Some(1));
        assert_eq!(ix.anchor_line("A.md", "nope").unwrap(), None);
    }
}
