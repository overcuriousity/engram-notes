//! Full-text and semantic retrieval, fused, with a divider where relevance falls.

pub mod fuse;
pub mod passages;
pub mod vector;

/// One note in a result list, with the passage or snippet that found it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Hit {
    pub path: String,
    pub title: String,
    /// HTML-safe; the full-text branch marks its matches with `<mark>`.
    pub snippet: String,
    pub heading: Option<String>,
    /// 1-based line in the file, for jumping to the passage.
    pub line: u32,
    /// Cosine of the best passage; `None` when only full-text found it.
    pub similarity: Option<f32>,
    /// The fused rank score, for ordering only.
    pub score: f64,
    pub past_divider: bool,
    pub primed: bool,
}

/// A note that did not match but is associated with one that did.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Associated {
    pub path: String,
    pub title: String,
    /// The hit that recalled it.
    pub via: String,
    /// The query that bound them, when one did.
    pub cue: Option<String>,
    pub strength: f64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct SearchResults {
    pub hits: Vec<Hit>,
    pub associated: Vec<Associated>,
}

use crate::Result;
use crate::config::{MemoryConfig, SearchConfig};
use crate::index::Index;
use std::collections::HashMap;

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Full-text and semantic retrieval fused into one list, with the divider
/// drawn, priming applied and associated notes spread beneath it.
///
/// `query_vec` is `None` while no model is loaded; the full-text branch alone
/// then answers, which is why search works during the first download.
pub fn hybrid(
    index: &Index,
    query: &str,
    query_vec: Option<&[f32]>,
    cfg: &SearchConfig,
    mem: &MemoryConfig,
    at: i64,
    limit: usize,
) -> Result<SearchResults> {
    if query.trim().is_empty() {
        return Ok(SearchResults::default());
    }
    let wide = limit * cfg.candidate_multiplier.max(1);
    let fts = index.search_fts(query, wide)?;
    // One entry per note: its best passage is the one the scan ranked first.
    let mut best: HashMap<String, vector::VecHit> = HashMap::new();
    let mut dense: Vec<String> = Vec::new();
    if let Some(q) = query_vec {
        for hit in index.search_vectors(q, wide)? {
            if best.contains_key(&hit.path) {
                continue;
            }
            dense.push(hit.path.clone());
            best.insert(hit.path.clone(), hit);
        }
        dense.truncate(wide);
    }
    let sparse: Vec<String> = fts.iter().map(|h| h.path.clone()).collect();
    let by_path: HashMap<&str, &crate::index::fts::FtsHit> =
        fts.iter().map(|h| (h.path.as_str(), h)).collect();
    let titles: HashMap<String, String> = index.titles()?.into_iter().collect();

    let mut hits: Vec<Hit> = fuse::rrf(&[dense, sparse], cfg.rrf_k)
        .into_iter()
        .take(limit)
        .map(|(path, score)| {
            let passage = best.get(&path);
            let text = by_path.get(path.as_str());
            Hit {
                title: titles.get(&path).cloned().unwrap_or_else(|| path.clone()),
                snippet: match text {
                    Some(h) => h.snippet.clone(),
                    None => escape_html(passage.map_or("", |p| p.text.as_str())),
                },
                heading: passage.map(|p| p.heading.clone()).filter(|h| !h.is_empty()),
                line: passage
                    .map(|p| p.line)
                    .or(text.map(|h| h.line))
                    .unwrap_or(1),
                similarity: passage.map(|p| p.similarity),
                score,
                past_divider: false,
                primed: false,
                path,
            }
        })
        .collect();

    if mem.enabled {
        let activation = index.activation_map(at, mem.activation_half_life_days)?;
        crate::memory::prime::prime(&mut hits, &activation, mem.prime_margin, mem.prime_lift);
    }
    fuse::mark_past_divider(&mut hits, cfg.cliff_factor, cfg.cliff_min_share);
    let associated = match mem.enabled {
        true => crate::memory::spread::spread(index, &hits, mem, at)?,
        false => vec![],
    };
    Ok(SearchResults { hits, associated })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{MemoryConfig, SearchConfig};
    use crate::embed::{Embedder, FakeEmbedder};
    use crate::index::Index;
    use crate::memory::EventKind;
    use crate::vault::Vault;
    use std::fs;

    fn vault_with_vectors() -> (tempfile::TempDir, Index, FakeEmbedder) {
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
        fs::write(d.path().join("Tea.md"), "# Tea\nleaves steep water").unwrap();
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

    #[test]
    fn both_branches_fuse_into_one_entry_per_note() {
        let (_d, ix, mut e) = vault_with_vectors();
        let q = e.embed_query("ownership").unwrap();
        let out = hybrid(
            &ix,
            "ownership",
            Some(&q),
            &SearchConfig::default(),
            &MemoryConfig::default(),
            0,
            10,
        )
        .unwrap();
        let paths: Vec<&str> = out.hits.iter().map(|h| h.path.as_str()).collect();
        assert_eq!(paths.iter().filter(|p| **p == "Rust.md").count(), 1);
        assert!(paths.contains(&"Rust.md") && paths.contains(&"Borrow.md"));
        assert!(
            out.hits[0].snippet.contains("<mark>"),
            "{}",
            out.hits[0].snippet
        );
        assert!(out.hits[0].similarity.is_some());
    }

    #[test]
    fn a_note_only_the_text_matched_has_no_similarity() {
        let (_d, ix, _e) = vault_with_vectors();
        let out = hybrid(
            &ix,
            "kettle",
            None,
            &SearchConfig::default(),
            &MemoryConfig::default(),
            0,
            10,
        )
        .unwrap();
        assert_eq!(out.hits[0].path, "Coffee.md");
        assert!(out.hits[0].similarity.is_none());
        assert!(out.hits.iter().all(|h| !h.past_divider));
    }

    #[test]
    fn memory_off_skips_priming_and_spread() {
        let (_d, mut ix, mut e) = vault_with_vectors();
        let mut cfg = MemoryConfig::default();
        ix.bump_assoc("Rust.md", "Tea.md", 5.0, Some("cue"), &cfg, 0)
            .unwrap();
        ix.record_event(EventKind::Open, Some("Coffee.md"), None, &cfg, 0)
            .unwrap();
        let q = e.embed_query("ownership").unwrap();
        cfg.enabled = false;
        let out = hybrid(
            &ix,
            "ownership",
            Some(&q),
            &SearchConfig::default(),
            &cfg,
            0,
            10,
        )
        .unwrap();
        assert!(out.associated.is_empty());
        assert!(out.hits.iter().all(|h| !h.primed));
    }

    #[test]
    fn memory_on_spreads_to_an_associated_note() {
        let (_d, mut ix, mut e) = vault_with_vectors();
        let cfg = MemoryConfig::default();
        ix.bump_assoc("Rust.md", "Tea.md", 5.0, Some("cue"), &cfg, 0)
            .unwrap();
        let q = e.embed_query("ownership").unwrap();
        let out = hybrid(
            &ix,
            "ownership rules",
            Some(&q),
            &SearchConfig::default(),
            &cfg,
            0,
            3,
        )
        .unwrap();
        assert!(out.hits.len() <= 3);
        assert_eq!(
            out.associated
                .iter()
                .map(|a| a.path.as_str())
                .collect::<Vec<_>>(),
            vec!["Tea.md"]
        );
        assert_eq!(out.associated[0].cue.as_deref(), Some("cue"));
    }

    #[test]
    fn an_empty_query_returns_nothing() {
        let (_d, ix, _e) = vault_with_vectors();
        let out = hybrid(
            &ix,
            "  ",
            None,
            &SearchConfig::default(),
            &MemoryConfig::default(),
            0,
            10,
        )
        .unwrap();
        assert_eq!(out, SearchResults::default());
    }
}
