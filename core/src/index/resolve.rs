//! Link targets to note paths: exact path, then basename anywhere; `.md`
//! implied; case-insensitive; the shortest path wins a tie.

use super::Index;
use crate::Result;
use rusqlite::OptionalExtension;

impl Index {
    pub fn resolve_target(&self, target: &str) -> Result<Option<String>> {
        resolve_in(&self.conn, target)
    }

    pub fn resolve_all(&mut self) -> Result<()> {
        let targets: Vec<String> = self
            .conn
            .prepare("SELECT DISTINCT target_raw FROM links")?
            .query_map([], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        let tx = self.conn.transaction()?;
        {
            let mut upd =
                tx.prepare_cached("UPDATE links SET target_path=?2 WHERE target_raw=?1")?;
            for raw in targets {
                let resolved = resolve_in(&tx, &raw)?;
                upd.execute(rusqlite::params![raw, resolved])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

fn resolve_in(conn: &rusqlite::Connection, target: &str) -> Result<Option<String>> {
    let t = crate::vault::normalize_rel(target).to_lowercase();
    let t = t.trim_end_matches('/');
    if t.is_empty() {
        return Ok(None);
    }
    let with_md = if t.ends_with(".md") {
        t.to_owned()
    } else {
        format!("{t}.md")
    };
    if let Some(p) = conn
        .query_row(
            "SELECT path FROM notes WHERE path_lower=?1",
            [&with_md],
            |r| r.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(Some(p));
    }
    let stem = with_md.rsplit('/').next().unwrap().trim_end_matches(".md");
    Ok(conn
        .query_row(
            "SELECT path FROM notes WHERE stem_lower=?1 ORDER BY length(path), path LIMIT 1",
            [stem],
            |r| r.get::<_, String>(0),
        )
        .optional()?)
}

#[cfg(test)]
mod tests {
    use crate::index::Index;
    use crate::vault::Vault;
    use std::fs;

    fn ix() -> (tempfile::TempDir, Index) {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("deep/er")).unwrap();
        fs::create_dir_all(d.path().join("other")).unwrap();
        fs::write(
            d.path().join("Home.md"),
            "[[Note]] [[deep/Note]] [[other/note]] [[home]] [[Ghost]] [x](deep/er/Note.md) [[Note#Sec|alias]]",
        )
        .unwrap();
        fs::write(d.path().join("deep/Note.md"), "").unwrap();
        fs::write(d.path().join("deep/er/Note.md"), "").unwrap();
        fs::write(d.path().join("other/Note.md"), "[[Home]]").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        (d, ix)
    }

    #[test]
    fn resolution_order_and_case() {
        let (_d, ix) = ix();
        let r = |t: &str| ix.resolve_target(t).unwrap();
        assert_eq!(r("deep/Note").as_deref(), Some("deep/Note.md"));
        assert_eq!(r("other/note").as_deref(), Some("other/Note.md"));
        assert_eq!(r("home").as_deref(), Some("Home.md"));
        assert_eq!(r("deep/er/Note.md").as_deref(), Some("deep/er/Note.md"));
        assert_eq!(r("Ghost"), None);
    }

    #[test]
    fn ambiguous_basename_takes_shortest_path() {
        let (_d, ix) = ix();
        assert_eq!(
            ix.resolve_target("Note").unwrap().as_deref(),
            Some("deep/Note.md")
        );
    }

    #[test]
    fn links_are_resolved_after_rebuild() {
        let (_d, ix) = ix();
        let out = ix.outgoing("Home.md").unwrap();
        let t: Vec<_> = out.iter().map(|l| l.target_path.as_deref()).collect();
        assert_eq!(
            t,
            vec![
                Some("deep/Note.md"),
                Some("deep/Note.md"),
                Some("other/Note.md"),
                Some("Home.md"),
                None,
                Some("deep/er/Note.md"),
                Some("deep/Note.md")
            ]
        );
        assert_eq!(out[6].alias.as_deref(), Some("alias"));
        assert_eq!(out[6].heading.as_deref(), Some("Sec"));
    }

    #[test]
    fn backlinks_and_unresolved() {
        let (_d, ix) = ix();
        let b = ix.backlinks("Home.md").unwrap();
        let src: Vec<_> = b.iter().map(|l| l.src_path.as_str()).collect();
        assert_eq!(src, vec!["Home.md", "other/Note.md"]);
        assert_eq!(b[1].context, "[[Home]]");
        let u = ix.unresolved().unwrap();
        assert_eq!(u.len(), 1);
        assert_eq!((u[0].target.as_str(), u[0].count), ("Ghost", 1));
    }

    #[test]
    fn creating_the_missing_note_resolves_the_link() {
        let (d, mut ix) = ix();
        fs::write(d.path().join("Ghost.md"), "").unwrap();
        let v = Vault::open(d.path()).unwrap();
        ix.update_file(&v, "Ghost.md").unwrap();
        assert!(ix.unresolved().unwrap().is_empty());
    }

    #[test]
    fn block_links_are_stored_and_anchors_found() {
        let d = tempfile::tempdir().unwrap();
        fs::write(
            d.path().join("T.md"),
            "---\na: 1\n---\n# Intro\ntext ^b1\n## Deep Part\n",
        )
        .unwrap();
        fs::write(d.path().join("S.md"), "[[T#^b1]]").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let out = ix.outgoing("S.md").unwrap();
        assert_eq!(out[0].block.as_deref(), Some("b1"));
        assert_eq!(out[0].heading, None);
        assert_eq!(ix.anchor_line("T.md", "^b1").unwrap(), Some(5));
        assert_eq!(ix.anchor_line("T.md", "deep part").unwrap(), Some(6));
        assert_eq!(ix.anchor_line("T.md", "Intro#Deep Part").unwrap(), Some(6));
        assert_eq!(ix.anchor_line("T.md", "^nope").unwrap(), None);
    }

    #[test]
    fn summaries_tags_properties_titles() {
        let d = tempfile::tempdir().unwrap();
        fs::write(
            d.path().join("P.md"),
            "---\ntitle: Pretty\nn: 3\n---\n#x #y",
        )
        .unwrap();
        fs::write(d.path().join("Q.md"), "#x").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let notes = ix.notes().unwrap();
        assert_eq!(
            notes.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(),
            vec!["Pretty", "Q"]
        );
        assert_eq!(ix.note("P.md").unwrap().unwrap().title, "Pretty");
        assert!(ix.note("nope.md").unwrap().is_none());
        let tags = ix.tags().unwrap();
        assert_eq!(
            tags.iter()
                .map(|t| (t.tag.as_str(), t.count))
                .collect::<Vec<_>>(),
            vec![("x", 2), ("y", 1)]
        );
        assert_eq!(ix.properties("P.md").unwrap()["n"], serde_json::json!(3));
        assert_eq!(
            ix.titles().unwrap(),
            vec![
                ("P.md".to_string(), "Pretty".to_string()),
                ("Q.md".to_string(), "Q".to_string())
            ]
        );
    }
}
