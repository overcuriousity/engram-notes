use super::{Index, RebuildStats};
use crate::Result;
use crate::parse::{LinkKind, ParsedNote, parse};
use crate::vault::{FileEntry, Vault};
use rusqlite::params;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".md")
        .or_else(|| name.strip_suffix(".MD"))
        .unwrap_or(name)
}

fn hash(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

// The source line the body starts on, so a link's line maps into `body`.
fn line_of_body(text: &str, body_offset: usize) -> i64 {
    text[..body_offset].bytes().filter(|b| *b == b'\n').count() as i64 + 1
}

fn value_type(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "checkbox",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(s) => {
            let b = s.as_bytes();
            if b.len() == 10 && b[4] == b'-' && b[7] == b'-' {
                "date"
            } else if b.len() >= 16 && b[4] == b'-' && b[10] == b'T' {
                "datetime"
            } else {
                "text"
            }
        }
        serde_json::Value::Array(_) => "list",
        serde_json::Value::Object(_) => "object",
    }
}

impl Index {
    pub fn rebuild(&mut self, vault: &Vault) -> Result<RebuildStats> {
        let mut stats = RebuildStats::default();
        let known: HashMap<String, (i64, i64)> = self
            .conn
            .prepare("SELECT path, mtime_ms, size FROM notes")?
            .query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?
            .collect::<std::result::Result<_, _>>()?;
        let mut seen = HashSet::new();
        let tx = self.conn.transaction()?;
        for entry in vault.walk()?.into_iter().filter(|e| e.is_markdown) {
            seen.insert(entry.path.clone());
            match known.get(&entry.path) {
                Some(&(m, s)) if m == entry.mtime_ms && s == entry.size as i64 => {
                    stats.unchanged += 1
                }
                Some(_) => {
                    let text = vault.read(&entry.path)?;
                    write_note(&tx, &entry, &text, &parse(&text))?;
                    stats.updated += 1;
                }
                None => {
                    let text = vault.read(&entry.path)?;
                    write_note(&tx, &entry, &text, &parse(&text))?;
                    stats.added += 1;
                }
            }
        }
        for path in known.keys().filter(|p| !seen.contains(*p)) {
            tx.execute("DELETE FROM notes WHERE path=?1", [path])?;
            stats.removed += 1;
        }
        tx.commit()?;
        // Nothing added, changed or removed leaves every resolution valid.
        if stats.added + stats.updated + stats.removed > 0 {
            self.resolve_all()?;
        }
        Ok(stats)
    }

    pub fn update_file(&mut self, vault: &Vault, rel: &str) -> Result<bool> {
        let Some(entry) = vault.stat(rel)? else {
            let existed = self
                .conn
                .execute("DELETE FROM notes WHERE path=?1", [rel])?
                > 0;
            if existed {
                self.resolve_all()?;
            }
            return Ok(existed);
        };
        if !entry.is_markdown {
            return Ok(false);
        }
        let text = vault.read(rel)?;
        let new_hash = hash(&text);
        let old_hash: Option<String> = self
            .conn
            .query_row("SELECT hash FROM notes WHERE path=?1", [rel], |r| r.get(0))
            .ok();
        if old_hash.as_deref() == Some(new_hash.as_str()) {
            self.conn.execute(
                "UPDATE notes SET mtime_ms=?2, size=?3 WHERE path=?1",
                params![rel, entry.mtime_ms, entry.size as i64],
            )?;
            return Ok(false);
        }
        let tx = self.conn.transaction()?;
        write_note(&tx, &entry, &text, &parse(&text))?;
        tx.commit()?;
        self.resolve_all()?;
        Ok(true)
    }

    pub fn remove_file(&mut self, rel: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM notes WHERE path=?1", [rel])?;
        self.resolve_all()
    }
}

fn write_note(
    tx: &rusqlite::Transaction,
    entry: &FileEntry,
    text: &str,
    note: &ParsedNote,
) -> Result<()> {
    let title = note
        .title
        .clone()
        .unwrap_or_else(|| stem(&entry.path).to_owned());
    tx.execute("DELETE FROM notes WHERE path=?1", [&entry.path])?;
    tx.execute(
        "INSERT INTO notes(path, path_lower, stem_lower, title, mtime_ms, size, hash, frontmatter, body, body_line)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            entry.path,
            entry.path.to_lowercase(),
            stem(&entry.path).to_lowercase(),
            title,
            entry.mtime_ms,
            entry.size as i64,
            hash(text),
            serde_json::Value::Object(note.frontmatter.clone()).to_string(),
            note.body,
            line_of_body(text, note.body_offset),
        ],
    )?;
    let mut ins = tx.prepare_cached(
        "INSERT INTO links(src_path, target_raw, target_path, kind, heading, block, alias, line, start, \"end\")
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    for l in &note.links {
        let kind = match l.kind {
            LinkKind::Wiki => "wiki",
            LinkKind::Markdown => "markdown",
            LinkKind::Embed => "embed",
        };
        ins.execute(params![
            entry.path,
            l.target,
            kind,
            l.heading,
            l.block,
            l.alias,
            l.line,
            l.start as i64,
            l.end as i64
        ])?;
    }
    let mut ins = tx.prepare_cached("INSERT INTO tags(path, tag) VALUES (?1, ?2)")?;
    for t in &note.tags {
        ins.execute(params![entry.path, t])?;
    }
    let mut ins = tx.prepare_cached(
        "INSERT INTO properties(path, key, value_json, value_type) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (k, v) in &note.frontmatter {
        ins.execute(params![entry.path, k, v.to_string(), value_type(v)])?;
    }
    let mut ins =
        tx.prepare_cached("INSERT INTO headings(path, level, text, line) VALUES (?1, ?2, ?3, ?4)")?;
    for h in &note.headings {
        ins.execute(params![entry.path, h.level, h.text, h.line])?;
    }
    let mut ins = tx.prepare_cached("INSERT INTO blocks(path, id, line) VALUES (?1, ?2, ?3)")?;
    for b in &note.blocks {
        ins.execute(params![entry.path, b.id, b.line])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::index::Index;
    use crate::vault::Vault;
    use std::fs;

    fn vault() -> (tempfile::TempDir, Vault) {
        let d = tempfile::tempdir().unwrap();
        fs::write(
            d.path().join("A.md"),
            "---\ntitle: Alpha\nstatus: open\n---\n# A\nlinks [[B]] and [[Missing]] #t1",
        )
        .unwrap();
        fs::create_dir_all(d.path().join("sub")).unwrap();
        fs::write(d.path().join("sub/B.md"), "back to [[A]]").unwrap();
        fs::write(d.path().join("pic.png"), "x").unwrap();
        let v = Vault::open(d.path()).unwrap();
        (d, v)
    }

    fn count(ix: &Index, sql: &str) -> i64 {
        ix.conn().query_row(sql, [], |r| r.get(0)).unwrap()
    }

    fn title(ix: &Index, path: &str) -> String {
        ix.conn()
            .query_row("SELECT title FROM notes WHERE path=?1", [path], |r| {
                r.get(0)
            })
            .unwrap()
    }

    #[test]
    fn rebuild_indexes_markdown_only() {
        let (_d, v) = vault();
        let mut ix = Index::open_in_memory().unwrap();
        let s = ix.rebuild(&v).unwrap();
        assert_eq!((s.added, s.updated, s.removed, s.unchanged), (2, 0, 0, 0));
        assert_eq!(count(&ix, "SELECT count(*) FROM notes"), 2);
        assert_eq!(count(&ix, "SELECT count(*) FROM links"), 3);
        assert_eq!(count(&ix, "SELECT count(*) FROM tags WHERE tag='t1'"), 1);
        assert_eq!(
            count(
                &ix,
                "SELECT count(*) FROM properties WHERE key='status' AND value_json='\"open\"'"
            ),
            1
        );
        assert_eq!(title(&ix, "A.md"), "Alpha");
        assert_eq!(title(&ix, "sub/B.md"), "B");
    }

    #[test]
    fn second_rebuild_is_unchanged_and_detects_edits_and_removals() {
        let (d, v) = vault();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let s = ix.rebuild(&v).unwrap();
        assert_eq!((s.added, s.updated, s.removed, s.unchanged), (0, 0, 0, 2));
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(d.path().join("sub/B.md"), "changed, no links").unwrap();
        fs::remove_file(d.path().join("A.md")).unwrap();
        let s = ix.rebuild(&v).unwrap();
        assert_eq!((s.added, s.updated, s.removed, s.unchanged), (0, 1, 1, 0));
        assert_eq!(count(&ix, "SELECT count(*) FROM links"), 0);
        assert_eq!(count(&ix, "SELECT count(*) FROM notes"), 1);
    }

    #[test]
    fn update_file_handles_change_and_disappearance() {
        let (d, v) = vault();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        assert!(!ix.update_file(&v, "A.md").unwrap());
        fs::write(d.path().join("A.md"), "new body").unwrap();
        assert!(ix.update_file(&v, "A.md").unwrap());
        assert_eq!(
            count(&ix, "SELECT count(*) FROM links WHERE src_path='A.md'"),
            0
        );
        fs::remove_file(d.path().join("A.md")).unwrap();
        assert!(ix.update_file(&v, "A.md").unwrap());
        assert_eq!(count(&ix, "SELECT count(*) FROM notes"), 1);
    }

    #[test]
    fn schema_version_mismatch_rebuilds_file() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("ix.db");
        {
            let ix = Index::open(&p).unwrap();
            ix.conn()
                .execute("UPDATE meta SET value='0' WHERE key='schema'", [])
                .unwrap();
            ix.conn()
                .execute(
                    "INSERT INTO notes VALUES ('x.md','x.md','x','x',0,0,'','{}','',1)",
                    [],
                )
                .unwrap();
        }
        let ix = Index::open(&p).unwrap();
        assert_eq!(count(&ix, "SELECT count(*) FROM notes"), 0);
    }

    #[test]
    fn corrupt_file_is_recreated() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("ix.db");
        fs::write(&p, vec![b'x'; 4096]).unwrap();
        assert!(Index::open(&p).is_err());
        let (ix, recreated) = Index::open_or_recreate(&p).unwrap();
        assert!(recreated);
        assert_eq!(count(&ix, "SELECT count(*) FROM notes"), 0);
        drop(ix);
        let (_ix, recreated) = Index::open_or_recreate(&p).unwrap();
        assert!(!recreated);
    }
}
