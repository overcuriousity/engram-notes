//! What `[[` completion and the passage picker list: spelling, then meaning.

use crate::Result;
use crate::config::{MemoryConfig, SearchConfig};
use crate::index::Index;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchKind {
    Text,
    Meaning,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LinkCandidate {
    pub path: String,
    pub title: String,
    pub kind: MatchKind,
    pub primed: bool,
}

fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".md").unwrap_or(name)
}

/// How well a spelling match answers the query, best first. The list is cut to
/// `limit`, so without a rank a vault whose paths all contain the query buries
/// the note actually named by it.
fn rank(title: &str, path: &str, q: &str) -> u8 {
    let (title, stem) = (title.to_lowercase(), stem(path).to_lowercase());
    if title == q || stem == q {
        0
    } else if title.starts_with(q) || stem.starts_with(q) {
        1
    } else if title.contains(q) || stem.contains(q) {
        2
    } else {
        3
    }
}

/// Notes whose title, stem or path contains the query, then notes retrieval
/// finds above the divider that spelling did not, up to `limit`.
pub fn link_candidates(
    index: &Index,
    query: &str,
    query_vec: Option<&[f32]>,
    cfg: &SearchConfig,
    mem: &MemoryConfig,
    at: i64,
    limit: usize,
) -> Result<Vec<LinkCandidate>> {
    let q = query.trim().to_lowercase();
    let mut out: Vec<LinkCandidate> = index
        .titles()?
        .into_iter()
        .filter(|(p, t)| {
            q.is_empty()
                || t.to_lowercase().contains(&q)
                || stem(p).to_lowercase().contains(&q)
                || p.to_lowercase().contains(&q)
        })
        .map(|(path, title)| LinkCandidate {
            path,
            title,
            kind: MatchKind::Text,
            primed: false,
        })
        .collect();
    // Stable, so notes of equal rank keep the index's title order.
    out.sort_by_key(|c| rank(&c.title, &c.path, &q));
    // Spelling alone can already fill the list, and retrieval is the expensive
    // half: the vector scan runs over every note, once per keystroke.
    if !q.is_empty() && out.len() < limit {
        let seen: HashSet<String> = out.iter().map(|c| c.path.clone()).collect();
        let found = super::hybrid_hits(index, query, query_vec, cfg, mem, at, limit)?;
        out.extend(
            found
                .into_iter()
                .filter(|h| !h.past_divider && !seen.contains(&h.path))
                .map(|h| LinkCandidate {
                    path: h.path,
                    title: h.title,
                    kind: MatchKind::Meaning,
                    primed: h.primed,
                }),
        );
    }
    out.truncate(limit);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::{Embedder, FakeEmbedder};
    use crate::vault::Vault;
    use std::fs;

    fn ix() -> (tempfile::TempDir, Index, FakeEmbedder) {
        let d = tempfile::tempdir().unwrap();
        fs::write(
            d.path().join("Rust.md"),
            "# Rust\nownership rules keep memory safe",
        )
        .unwrap();
        fs::write(
            d.path().join("Borrow.md"),
            "# Borrow\nownership and borrowing explained",
        )
        .unwrap();
        fs::write(
            d.path().join("Coffee.md"),
            "# Coffee\nkettle water grind beans",
        )
        .unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let mut e = FakeEmbedder::new(64);
        ix.set_model_id(&e.id()).unwrap();
        let pending = ix.pending_vectors(1000).unwrap();
        let texts: Vec<String> = pending.iter().map(|p| p.text.clone()).collect();
        let vecs = e.embed_documents(&texts).unwrap();
        let rows: Vec<(String, Vec<f32>)> = pending.into_iter().map(|p| p.hash).zip(vecs).collect();
        ix.put_vectors(&rows).unwrap();
        (d, ix, e)
    }

    fn run(ix: &Index, e: &mut FakeEmbedder, q: &str) -> Vec<LinkCandidate> {
        let v = e.embed_query(q).unwrap();
        link_candidates(
            ix,
            q,
            Some(&v),
            &SearchConfig::default(),
            &MemoryConfig::default(),
            0,
            10,
        )
        .unwrap()
    }

    #[test]
    fn spelling_comes_first_and_meaning_fills_in() {
        let (_d, ix, mut e) = ix();
        let out = run(&ix, &mut e, "ownership");
        let meaning: Vec<&str> = out
            .iter()
            .filter(|c| c.kind == MatchKind::Meaning)
            .map(|c| c.path.as_str())
            .collect();
        assert!(
            meaning.contains(&"Rust.md") && meaning.contains(&"Borrow.md"),
            "{out:?}"
        );
        let out = run(&ix, &mut e, "borrow");
        assert_eq!(out[0].path, "Borrow.md");
        assert_eq!(out[0].kind, MatchKind::Text);
        assert_eq!(out.iter().filter(|c| c.path == "Borrow.md").count(), 1);
    }

    #[test]
    fn an_empty_query_lists_every_note_by_spelling() {
        let (_d, ix, mut e) = ix();
        let out = run(&ix, &mut e, "  ");
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|c| c.kind == MatchKind::Text));
    }

    #[test]
    fn the_note_named_by_the_query_survives_the_cut() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("Archive/rust")).unwrap();
        for i in 0..30 {
            fs::write(
                d.path().join(format!("Archive/rust/n{i}.md")),
                format!("# Note {i}\nnothing to do with it"),
            )
            .unwrap();
        }
        fs::write(d.path().join("Rust.md"), "# Rust\nownership").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let out = link_candidates(
            &ix,
            "rust",
            None,
            &SearchConfig::default(),
            &MemoryConfig::default(),
            0,
            20,
        )
        .unwrap();
        assert_eq!(out.len(), 20);
        assert_eq!(out[0].path, "Rust.md");
    }

    #[test]
    fn without_a_model_the_full_text_branch_still_answers() {
        let (_d, ix, _e) = ix();
        let out = link_candidates(
            &ix,
            "kettle",
            None,
            &SearchConfig::default(),
            &MemoryConfig::default(),
            0,
            10,
        )
        .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "Coffee.md");
        assert_eq!(out[0].kind, MatchKind::Meaning);
    }
}
