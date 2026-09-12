//! Renaming a note means rewriting every link that pointed at it.

use crate::Result;
use crate::index::Index;
use crate::parse::{LinkKind, parse};
use crate::vault::{Vault, normalize_rel};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RenamePlan {
    pub from: String,
    pub to: String,
    pub affected: Vec<String>,
}

fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".md").unwrap_or(name)
}

pub fn plan_rename(index: &Index, from: &str, to: &str) -> Result<RenamePlan> {
    let from = normalize_rel(from);
    let to = normalize_rel(to);
    let mut affected: Vec<String> = index
        .backlinks(&from)?
        .into_iter()
        .map(|l| l.src_path)
        .filter(|p| *p != from)
        .collect();
    affected.dedup();
    Ok(RenamePlan { from, to, affected })
}

pub fn apply_rename(vault: &Vault, index: &mut Index, plan: &RenamePlan) -> Result<()> {
    let new_stem = stem(&plan.to).to_lowercase();
    let unique = !index
        .titles()?
        .iter()
        .any(|(p, _)| *p != plan.from && stem(p).to_lowercase() == new_stem);
    vault.rename(&plan.from, &plan.to)?;
    // Self-links inside the moved note point at its old name too.
    for path in plan.affected.iter().chain(std::iter::once(&plan.to)) {
        let text = vault.read(path)?;
        let out = rewrite_links(&text, &plan.from, &plan.to, unique);
        if out != text {
            vault.write(path, &out)?;
        }
    }
    index.remove_file(&plan.from)?;
    index.update_file(vault, &plan.to)?;
    for path in &plan.affected {
        index.update_file(vault, path)?;
    }
    Ok(())
}

/// Does `target`, as written in a link, name `old_path`? The resolver's rule
/// applied to one known note: exact path or bare stem, case-insensitive.
fn names(target: &str, old_path: &str) -> bool {
    let t = normalize_rel(target).to_lowercase();
    let t = t.strip_suffix(".md").unwrap_or(&t);
    let old = old_path.to_lowercase();
    let old = old.strip_suffix(".md").unwrap_or(&old);
    t == old || t == stem(old)
}

pub fn rewrite_links(
    text: &str,
    old_path: &str,
    new_path: &str,
    new_is_unique_stem: bool,
) -> String {
    let note = parse(text);
    let new_wiki = if new_is_unique_stem {
        stem(new_path).to_owned()
    } else {
        new_path.strip_suffix(".md").unwrap_or(new_path).to_owned()
    };
    let mut out = String::with_capacity(text.len());
    let mut pos = 0;
    for l in note.links.iter().filter(|l| names(&l.target, old_path)) {
        out.push_str(&text[pos..l.start]);
        let original = &text[l.start..l.end];
        match l.kind {
            LinkKind::Wiki | LinkKind::Embed => {
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
                out.push_str(&format!("{bang}[[{new_wiki}{fragment}{alias}]]"));
            }
            LinkKind::Markdown => {
                let open = original.rfind('(').unwrap_or(0);
                out.push_str(&original[..=open]);
                out.push_str(&new_path.replace(' ', "%20"));
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

    #[test]
    fn rewrite_keeps_alias_heading_and_other_text() {
        let text = "see [[Old|shown]] and [[Old#Sec]] and ![[Old]] and [[Older]] and [m](sub/Old.md) `[[Old]]`";
        let out = rewrite_links(text, "sub/Old.md", "New Name.md", true);
        assert_eq!(
            out,
            "see [[New Name|shown]] and [[New Name#Sec]] and ![[New Name]] and [[Older]] and [m](New%20Name.md) `[[Old]]`"
        );
    }

    #[test]
    fn rewrite_uses_full_path_when_stem_is_ambiguous() {
        let out = rewrite_links("[[Old]]", "Old.md", "a/Note.md", false);
        assert_eq!(out, "[[a/Note]]");
    }

    #[test]
    fn rewrite_keeps_block_fragment() {
        let out = rewrite_links("[[Old#^b1|see]]", "Old.md", "New.md", true);
        assert_eq!(out, "[[New#^b1|see]]");
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
        let plan = plan_rename(&ix, "Old.md", "sub/New.md").unwrap();
        assert_eq!(plan.affected, vec!["Ref.md"]);
        apply_rename(&v, &mut ix, &plan).unwrap();
        assert_eq!(v.read("Ref.md").unwrap(), "link [[New]]");
        assert_eq!(v.read("sub/New.md").unwrap(), "I am old");
        assert!(ix.note("Old.md").unwrap().is_none());
        assert_eq!(ix.backlinks("sub/New.md").unwrap().len(), 1);
        assert!(ix.unresolved().unwrap().is_empty());
    }
}
