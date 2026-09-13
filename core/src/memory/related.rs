//! What the open note is related to: learned links, near passages, and the
//! links it could have.

use crate::Result;
use crate::config::MemoryConfig;
use crate::index::Index;
use crate::search::Associated;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SimilarNote {
    pub path: String,
    pub title: String,
    pub heading: String,
    pub text: String,
    pub similarity: f32,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Related {
    pub associated: Vec<Associated>,
    pub similar: Vec<SimilarNote>,
    /// Obsidian's unlinked mentions, extended by meaning.
    pub suggested: Vec<SimilarNote>,
}

pub fn related(
    index: &Index,
    path: &str,
    text: &str,
    cfg: &MemoryConfig,
    at: i64,
    limit: usize,
) -> Result<Related> {
    let titles: std::collections::HashMap<String, String> = index.titles()?.into_iter().collect();
    let associated = match cfg.enabled {
        true => index
            .assoc_from(&[path.to_owned()], at, cfg)?
            .into_iter()
            .map(|row| Associated {
                title: titles
                    .get(&row.other)
                    .cloned()
                    .unwrap_or_else(|| row.other.clone()),
                path: row.other,
                via: row.via,
                cue: row.cue,
                strength: row.value,
            })
            .collect(),
        false => vec![],
    };
    let similar: Vec<SimilarNote> = index
        .similar_to(&[path.to_owned()], limit)?
        .into_iter()
        .map(|h| SimilarNote {
            title: titles
                .get(&h.path)
                .cloned()
                .unwrap_or_else(|| h.path.clone()),
            path: h.path,
            heading: h.heading,
            text: h.text,
            similarity: h.similarity,
        })
        .collect();
    let linked: HashSet<String> = index
        .outgoing(path)?
        .into_iter()
        .filter_map(|l| l.target_path)
        .collect();
    let lower = text.to_lowercase();
    let suggested = similar
        .iter()
        .filter(|s| !linked.contains(&s.path) && !lower.contains(&s.title.to_lowercase()))
        .cloned()
        .collect();
    Ok(Related {
        associated,
        similar,
        suggested,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MemoryConfig;
    use crate::embed::{Embedder, FakeEmbedder};
    use crate::index::Index;
    use crate::vault::Vault;
    use std::fs;

    fn ix() -> (tempfile::TempDir, Index) {
        let d = tempfile::tempdir().unwrap();
        fs::write(
            d.path().join("Rust.md"),
            "# Rust\nownership rules keep memory safe. see [[Borrow]]",
        )
        .unwrap();
        fs::write(
            d.path().join("Borrow.md"),
            "# Borrow\nownership rules and borrowing",
        )
        .unwrap();
        fs::write(
            d.path().join("Lifetimes.md"),
            "# Lifetimes\nownership rules over time",
        )
        .unwrap();
        fs::write(d.path().join("Coffee.md"), "# Coffee\nkettle grind beans").unwrap();
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
        (d, ix)
    }

    #[test]
    fn similar_lists_other_notes_nearest_first() {
        let (_d, ix) = ix();
        let text = "# Rust\nownership rules keep memory safe. see [[Borrow]]";
        let r = related(&ix, "Rust.md", text, &MemoryConfig::default(), 0, 10).unwrap();
        assert!(r.similar.iter().all(|s| s.path != "Rust.md"));
        assert_eq!(r.similar[0].path, "Borrow.md");
        assert_eq!(r.similar[0].heading, "Borrow");
    }

    #[test]
    fn a_note_already_linked_is_not_suggested() {
        let (_d, ix) = ix();
        let text = "# Rust\nownership rules keep memory safe. see [[Borrow]]";
        let r = related(&ix, "Rust.md", text, &MemoryConfig::default(), 0, 10).unwrap();
        assert!(r.suggested.iter().all(|s| s.path != "Borrow.md"));
        assert!(r.suggested.iter().any(|s| s.path == "Lifetimes.md"));
    }

    #[test]
    fn a_title_the_text_already_names_is_not_suggested() {
        let (_d, ix) = ix();
        let text = "# Rust\nownership rules; lifetimes matter too";
        let r = related(&ix, "Rust.md", text, &MemoryConfig::default(), 0, 10).unwrap();
        assert!(r.suggested.iter().all(|s| s.path != "Lifetimes.md"));
    }

    #[test]
    fn associated_shows_a_learned_link_and_memory_off_hides_it() {
        let (_d, mut ix) = ix();
        let mut cfg = MemoryConfig::default();
        ix.bump_assoc("Rust.md", "Coffee.md", 5.0, Some("why"), &cfg, 0)
            .unwrap();
        let r = related(&ix, "Rust.md", "x", &cfg, 0, 10).unwrap();
        assert_eq!(r.associated[0].path, "Coffee.md");
        assert_eq!(r.associated[0].cue.as_deref(), Some("why"));
        cfg.enabled = false;
        let off = related(&ix, "Rust.md", "x", &cfg, 0, 10).unwrap();
        assert!(off.associated.is_empty());
        assert!(!off.similar.is_empty(), "similar is shown with memory off");
    }
}
