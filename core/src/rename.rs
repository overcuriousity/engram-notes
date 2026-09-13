//! Renaming or moving a file or folder means rewriting every link that
//! pointed at what moved.

use crate::Result;
use crate::index::Index;
use crate::parse::{LinkKind, parse};
use crate::vault::{Vault, normalize_rel};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RenamePlan {
    pub from: String,
    pub to: String,
    /// Files outside the move whose links change, by their current path.
    pub affected: Vec<String>,
}

/// One file's old and new path, and whether its new name is unique in the
/// vault so a wikilink can stay a bare name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Move {
    pub from: String,
    pub to: String,
    pub unique: bool,
}

fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".md").unwrap_or(name)
}

fn is_note(path: &str) -> bool {
    path.to_lowercase().ends_with(".md")
}

fn clean(path: &str) -> String {
    normalize_rel(path).trim_end_matches('/').to_owned()
}

// A folder moves every file below it.
fn moves(vault: &Vault, from: &str, to: &str) -> Result<Vec<Move>> {
    let files = vault.walk()?;
    let pairs: Vec<(String, String)> = if vault.abs(from).is_dir() {
        let prefix = format!("{from}/");
        files
            .iter()
            .filter_map(|e| {
                let rest = e.path.strip_prefix(&prefix)?;
                Some((e.path.clone(), format!("{to}/{rest}")))
            })
            .collect()
    } else {
        vec![(from.to_owned(), to.to_owned())]
    };
    Ok(pairs
        .into_iter()
        .map(|(from, to)| {
            let s = stem(&to).to_lowercase();
            let unique = !files
                .iter()
                .any(|e| e.path != from && stem(&e.path).to_lowercase() == s);
            Move { from, to, unique }
        })
        .collect())
}

// Attachments never resolve in the index, so links to them match by name.
fn referrers(index: &Index, path: &str) -> Result<Vec<String>> {
    if is_note(path) {
        return Ok(index
            .backlinks(path)?
            .into_iter()
            .map(|l| l.src_path)
            .collect());
    }
    let mut stmt = index
        .conn()
        .prepare("SELECT src_path, target_raw FROM links WHERE target_path IS NULL")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (src, raw) = row?;
        if names(&raw, path) {
            out.push(src);
        }
    }
    Ok(out)
}

pub fn plan_rename(vault: &Vault, index: &Index, from: &str, to: &str) -> Result<RenamePlan> {
    let (from, to) = (clean(from), clean(to));
    let moved: HashSet<String> = moves(vault, &from, &to)?
        .into_iter()
        .map(|m| m.from)
        .collect();
    let mut affected = Vec::new();
    for path in &moved {
        affected.extend(referrers(index, path)?);
    }
    affected.retain(|p| !moved.contains(p));
    affected.sort();
    affected.dedup();
    Ok(RenamePlan { from, to, affected })
}

pub fn apply_rename(vault: &Vault, index: &mut Index, plan: &RenamePlan) -> Result<()> {
    let moves = moves(vault, &plan.from, &plan.to)?;
    vault.rename(&plan.from, &plan.to)?;
    // Moved notes are rewritten too: they may name each other or themselves.
    for m in &moves {
        if is_note(&m.to) {
            index.move_memory(&m.from, &m.to)?;
        }
    }
    let mut files: Vec<String> = moves
        .iter()
        .filter(|m| is_note(&m.to))
        .map(|m| m.to.clone())
        .collect();
    files.extend(plan.affected.iter().cloned());
    files.sort();
    files.dedup();
    for path in &files {
        let text = vault.read(path)?;
        let out = rewrite_links(&text, &moves);
        if out != text {
            vault.write(path, &out)?;
        }
    }
    // One rebuild re-parses what moved or changed and resolves links once.
    index.rebuild(vault)?;
    Ok(())
}

/// Does `target`, as written in a link, name `old_path`? The resolver's rule
/// applied to one known file: exact path or bare name, case-insensitive.
fn names(target: &str, old_path: &str) -> bool {
    let t = normalize_rel(target).to_lowercase();
    let t = t.strip_suffix(".md").unwrap_or(&t);
    let old = old_path.to_lowercase();
    let old = old.strip_suffix(".md").unwrap_or(&old);
    t == old || t == stem(old)
}

pub fn rewrite_links(text: &str, moves: &[Move]) -> String {
    let note = parse(text);
    let mut out = String::with_capacity(text.len());
    let mut pos = 0;
    for l in &note.links {
        let Some(m) = moves.iter().find(|m| names(&l.target, &m.from)) else {
            continue;
        };
        out.push_str(&text[pos..l.start]);
        let original = &text[l.start..l.end];
        match l.kind {
            LinkKind::Wiki | LinkKind::Embed => {
                let name = if m.unique {
                    stem(&m.to)
                } else {
                    m.to.strip_suffix(".md").unwrap_or(&m.to)
                };
                let bang = if l.kind == LinkKind::Embed { "!" } else { "" };
                let fragment = match (&l.heading, &l.block) {
                    (Some(h), _) => format!("#{h}"),
                    (None, Some(b)) => format!("#^{b}"),
                    (None, None) => String::new(),
                };
                let alias = l
                    .alias
                    .as_ref()
                    .map(|a| format!("|{a}"))
                    .unwrap_or_default();
                out.push_str(&format!("{bang}[[{name}{fragment}{alias}]]"));
            }
            LinkKind::Markdown => {
                let open = original.rfind('(').unwrap_or(0);
                out.push_str(&original[..=open]);
                out.push_str(&m.to.replace(' ', "%20"));
                out.push(')');
            }
        }
        pos = l.end;
    }
    out.push_str(&text[pos..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn mv(from: &str, to: &str, unique: bool) -> Move {
        Move {
            from: from.into(),
            to: to.into(),
            unique,
        }
    }

    #[test]
    fn rewrite_keeps_alias_heading_and_other_text() {
        let text = "see [[Old|shown]] and [[Old#Sec]] and ![[Old]] and [[Older]] and [m](sub/Old.md) `[[Old]]`";
        let out = rewrite_links(text, &[mv("sub/Old.md", "New Name.md", true)]);
        assert_eq!(
            out,
            "see [[New Name|shown]] and [[New Name#Sec]] and ![[New Name]] and [[Older]] and [m](New%20Name.md) `[[Old]]`"
        );
    }

    #[test]
    fn rewrite_uses_full_path_when_stem_is_ambiguous() {
        let out = rewrite_links("[[Old]]", &[mv("Old.md", "a/Note.md", false)]);
        assert_eq!(out, "[[a/Note]]");
    }

    #[test]
    fn rewrite_keeps_block_fragment() {
        let out = rewrite_links("[[Old#^b1|see]]", &[mv("Old.md", "New.md", true)]);
        assert_eq!(out, "[[New#^b1|see]]");
    }

    fn indexed(files: &[(&str, &str)]) -> (tempfile::TempDir, Vault, Index) {
        let d = tempfile::tempdir().unwrap();
        for (path, text) in files {
            let p = d.path().join(path);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        }
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        (d, v, ix)
    }

    #[test]
    fn renaming_an_attachment_rewrites_its_embeds() {
        let (_d, v, mut ix) = indexed(&[("pic.png", "p"), ("N.md", "![[pic.png|300]]")]);
        let plan = plan_rename(&v, &ix, "pic.png", "img/photo.png").unwrap();
        assert_eq!(plan.affected, vec!["N.md"]);
        apply_rename(&v, &mut ix, &plan).unwrap();
        assert_eq!(v.read("N.md").unwrap(), "![[photo.png|300]]");
        assert!(v.stat("img/photo.png").unwrap().is_some());
    }

    #[test]
    fn folder_move_takes_every_file_and_rewrites_path_links() {
        let (d, v, mut ix) = indexed(&[
            ("old/A.md", "[[old/deep/B]] ![[pic.png]]"),
            ("old/deep/B.md", "b"),
            ("old/pic.png", "p"),
            ("Ref.md", "[[old/A]] [[B]] ![[old/pic.png]]"),
            ("Other.md", "nothing"),
        ]);
        let plan = plan_rename(&v, &ix, "old", "new/place").unwrap();
        assert_eq!(plan.affected, vec!["Ref.md"]);
        apply_rename(&v, &mut ix, &plan).unwrap();
        assert_eq!(v.read("Ref.md").unwrap(), "[[A]] [[B]] ![[pic.png]]");
        assert_eq!(v.read("new/place/A.md").unwrap(), "[[B]] ![[pic.png]]");
        assert!(v.stat("new/place/pic.png").unwrap().is_some());
        assert!(!d.path().join("old").exists());
        assert!(ix.note("old/A.md").unwrap().is_none());
        assert_eq!(ix.backlinks("new/place/deep/B.md").unwrap().len(), 2);
        assert!(ix.unresolved().unwrap().is_empty());
    }

    #[test]
    fn plan_and_apply_move_the_file_and_fix_referrers() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("Old.md"), "I am old").unwrap();
        fs::write(d.path().join("Ref.md"), "link [[Old]]").unwrap();
        fs::write(d.path().join("Unrelated.md"), "[[Ref]]").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let plan = plan_rename(&v, &ix, "Old.md", "sub/New.md").unwrap();
        assert_eq!(plan.affected, vec!["Ref.md"]);
        apply_rename(&v, &mut ix, &plan).unwrap();
        assert_eq!(v.read("Ref.md").unwrap(), "link [[New]]");
        assert_eq!(v.read("sub/New.md").unwrap(), "I am old");
        assert!(ix.note("Old.md").unwrap().is_none());
        assert_eq!(ix.backlinks("sub/New.md").unwrap().len(), 1);
        assert!(ix.unresolved().unwrap().is_empty());
    }

    #[test]
    fn a_rename_carries_memory_to_the_new_path() {
        let (_d, v, mut ix) = indexed(&[("Old.md", "I am old"), ("Ref.md", "[[Old]]")]);
        let cfg = crate::config::MemoryConfig::default();
        ix.record_event(
            crate::memory::EventKind::Open,
            Some("Old.md"),
            None,
            &cfg,
            0,
        )
        .unwrap();
        let plan = plan_rename(&v, &ix, "Old.md", "sub/New.md").unwrap();
        apply_rename(&v, &mut ix, &plan).unwrap();
        let act = ix.activation_map(0, 30.0).unwrap();
        assert!(act.contains_key("sub/New.md"), "{act:?}");
        assert!(!act.contains_key("Old.md"));
    }
}
