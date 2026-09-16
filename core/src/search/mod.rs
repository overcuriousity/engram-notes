//! Full-text and semantic retrieval, fused, with a divider where relevance falls.

pub mod candidates;
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
    /// 1-based line in the file. A note is one passage since 0.5, so this is
    /// where its body starts and not where the match sits.
    pub line: u32,
    /// Cosine of the best passage; `None` when only full-text found it.
    pub similarity: Option<f32>,
    /// The cross-encoder's score in `(0, 1)`, where reranking ran.
    pub rerank: Option<f32>,
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

/// A result row is one line, and a passage is the whole note since 0.5, so
/// anything showing passage text shows a lead instead of the body.
const LEAD_CHARS: usize = 200;

/// What the cross-encoder reads of a passage. Its cost is linear in characters
/// times pairs, and a note's important part is at its beginning: 20 pairs of
/// this length are one search's budget on a modest CPU (`docs/memory.md`).
pub const RERANK_CHARS: usize = 600;

/// The first line's worth of `text`, flattened, cut on a word boundary.
pub(crate) fn lead(text: &str) -> String {
    head(text, LEAD_CHARS)
}

/// The first `chars` of `text`, flattened, cut on a word boundary with an
/// ellipsis where something was cut.
fn head(text: &str, chars: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let Some((hard, _)) = flat.char_indices().nth(chars) else {
        return flat;
    };
    let at = flat[..hard].rfind(' ').unwrap_or(hard);
    format!("{}…", flat[..at].trim_end())
}

/// Full-text and semantic retrieval fused into one list, with the divider
/// drawn, priming applied and associated notes spread beneath it.
///
/// `query_vec` is `None` while no model is loaded; the full-text branch alone
/// then answers, which is why search works while the models load.
// Two model handles, two configs, a clock and a limit: each is a distinct
// input the caller owns, and a struct would only rename the list.
#[allow(clippy::too_many_arguments)]
pub fn hybrid(
    index: &Index,
    query: &str,
    query_vec: Option<&[f32]>,
    reranker: Option<&mut dyn crate::embed::Reranker>,
    cfg: &SearchConfig,
    mem: &MemoryConfig,
    at: i64,
    limit: usize,
) -> Result<SearchResults> {
    let hits = hybrid_hits(index, query, query_vec, reranker, cfg, mem, at, limit)?;
    let associated = match mem.enabled && !hits.is_empty() {
        true => crate::memory::spread::spread(index, &hits, mem, at)?,
        false => vec![],
    };
    Ok(SearchResults { hits, associated })
}

/// `hybrid` without the spread: what link completion wants, since it lists
/// notes the query itself found and never their associates.
// Two model handles, two configs, a clock and a limit: each is a distinct
// input the caller owns, and a struct would only rename the list.
#[allow(clippy::too_many_arguments)]
pub fn hybrid_hits(
    index: &Index,
    query: &str,
    query_vec: Option<&[f32]>,
    reranker: Option<&mut dyn crate::embed::Reranker>,
    cfg: &SearchConfig,
    mem: &MemoryConfig,
    at: i64,
    limit: usize,
) -> Result<Vec<Hit>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
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

    // With a reranker, more than `limit` fused entries are built so a note at
    // fused rank 15 can be lifted into a list of 10.
    let build = match reranker.is_some() {
        true => limit.max(cfg.rerank_n),
        false => limit,
    };
    let mut hits: Vec<Hit> = fuse::rrf(&[dense, sparse], cfg.rrf_k)
        .into_iter()
        .take(build)
        .map(|(path, score)| {
            let passage = best.get(&path);
            let text = by_path.get(path.as_str());
            Hit {
                title: titles.get(&path).cloned().unwrap_or_else(|| path.clone()),
                snippet: match text {
                    Some(h) => h.snippet.clone(),
                    None => escape_html(&lead(passage.map_or("", |p| p.text.as_str()))),
                },
                line: passage
                    .map(|p| p.line)
                    .or(text.map(|h| h.line))
                    .unwrap_or(1),
                similarity: passage.map(|p| p.similarity),
                rerank: None,
                score,
                past_divider: false,
                primed: false,
                path,
            }
        })
        .collect();

    if let Some(r) = reranker {
        rerank(index, query, r, &mut hits, &best, cfg.rerank_n)?;
    }
    hits.truncate(limit);

    if mem.enabled {
        let activation = index.activation_map(at, mem.activation_half_life_days)?;
        crate::memory::prime::prime(&mut hits, &activation, mem.prime_margin, mem.prime_lift);
    }
    fuse::mark_past_divider(&mut hits, cfg);
    Ok(hits)
}

/// Rescore the first `n` hits with the cross-encoder and put them in its
/// order; the rest keep fusion order beneath. A reranker that fails leaves the
/// list as it was: the failure is the shell's to report, not the search's.
fn rerank(
    index: &Index,
    query: &str,
    reranker: &mut dyn crate::embed::Reranker,
    hits: &mut [Hit],
    best: &HashMap<String, vector::VecHit>,
    n: usize,
) -> Result<()> {
    let n = n.min(hits.len());
    if n == 0 {
        return Ok(());
    }
    let mut texts = Vec::with_capacity(n);
    for h in &hits[..n] {
        let text = match best.get(&h.path) {
            Some(p) => p.text.clone(),
            None => index.passage_text(&h.path)?.unwrap_or_default(),
        };
        texts.push(head(&text, RERANK_CHARS));
    }
    let Ok(scores) = reranker.score(query, &texts) else {
        return Ok(());
    };
    if scores.len() != n {
        return Ok(());
    }
    for (h, s) in hits[..n].iter_mut().zip(scores) {
        h.rerank = Some(s);
    }
    // Stable, so equal scores keep fusion order.
    hits[..n].sort_by(|a, b| b.rerank.unwrap().total_cmp(&a.rerank.unwrap()));
    Ok(())
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

    #[test]
    fn a_lead_is_one_line_and_cut_on_a_word() {
        assert_eq!(lead("a\n\nb  c"), "a b c");
        // Plain text: a snippet is escaped where it is built, a related row is not.
        assert_eq!(lead("<b>&"), "<b>&");
        let long = lead(&"word ".repeat(100));
        assert!(long.ends_with("word…"), "{long}");
        assert!(long.chars().count() <= LEAD_CHARS + 1, "{}", long.len());
    }

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
            None,
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
            None,
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
            None,
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
            None,
            &SearchConfig::default(),
            &MemoryConfig::default(),
            0,
            10,
        )
        .unwrap();
        assert_eq!(out, SearchResults::default());
    }
    /// Scores by a fixed table, so the test decides the order the reranker wants.
    struct Table(std::collections::HashMap<&'static str, f32>);
    impl crate::embed::Reranker for Table {
        fn id(&self) -> String {
            "table".into()
        }
        fn score(&mut self, _q: &str, docs: &[String]) -> crate::Result<Vec<f32>> {
            Ok(docs
                .iter()
                .map(|d| {
                    self.0
                        .iter()
                        .find(|(k, _)| d.contains(**k))
                        .map_or(0.0, |(_, v)| *v)
                })
                .collect())
        }
    }

    fn plain(ix: &Index, q: &str, r: Option<&mut dyn crate::embed::Reranker>) -> SearchResults {
        hybrid(
            ix,
            q,
            None,
            r,
            &SearchConfig::default(),
            &MemoryConfig::default(),
            0,
            10,
        )
        .unwrap()
    }

    #[test]
    fn the_reranker_reorders_the_top_hits_and_marks_their_scores() {
        let (_d, ix, _e) = vault_with_vectors();
        // Full text alone finds Coffee (kettle water grind beans) and Tea
        // (leaves steep water) for "water"; the table prefers whichever fusion
        // put second, so the order can only come from the scores.
        let second = plain(&ix, "water", None).hits[1].path.clone();
        let (hi, lo) = match second.as_str() {
            "Tea.md" => ("steep", "kettle"),
            _ => ("kettle", "steep"),
        };
        let mut table = Table([(hi, 0.9f32), (lo, 0.05f32)].into_iter().collect());
        let out = plain(&ix, "water", Some(&mut table));
        let paths: Vec<&str> = out.hits.iter().map(|h| h.path.as_str()).collect();
        assert_eq!(paths[0], second, "{paths:?}");
        assert_eq!(paths.len(), 2);
        assert_eq!(out.hits[0].rerank, Some(0.9));
        assert_eq!(out.hits[1].rerank, Some(0.05));
        // The divider reads the rerank score: 0.05 is under the floor.
        assert!(!out.hits[0].past_divider);
        assert!(out.hits[1].past_divider);
        // Cosine is untouched: nothing dense ran.
        assert!(out.hits.iter().all(|h| h.similarity.is_none()));
    }

    #[test]
    fn without_a_reranker_hits_carry_no_rerank_score() {
        let (_d, ix, _e) = vault_with_vectors();
        let out = plain(&ix, "water", None);
        assert_eq!(out.hits.len(), 2);
        assert!(out.hits.iter().all(|h| h.rerank.is_none()));
    }

    #[test]
    fn a_failing_reranker_leaves_fusion_order_and_no_scores() {
        struct Broken;
        impl crate::embed::Reranker for Broken {
            fn id(&self) -> String {
                "broken".into()
            }
            fn score(&mut self, _: &str, _: &[String]) -> crate::Result<Vec<f32>> {
                Err(crate::Error::Embed("down".into()))
            }
        }
        let (_d, ix, _e) = vault_with_vectors();
        let mut broken = Broken;
        let out = plain(&ix, "water", Some(&mut broken));
        assert_eq!(out.hits.len(), 2);
        assert!(out.hits.iter().all(|h| h.rerank.is_none()));
    }

    #[test]
    fn the_reranker_reads_the_lead_of_a_passage() {
        struct Lengths(Vec<usize>);
        impl crate::embed::Reranker for Lengths {
            fn id(&self) -> String {
                "lengths".into()
            }
            fn score(&mut self, _: &str, docs: &[String]) -> crate::Result<Vec<f32>> {
                self.0 = docs.iter().map(|d| d.chars().count()).collect();
                Ok(vec![0.5; docs.len()])
            }
        }
        let d = tempfile::tempdir().unwrap();
        fs::write(
            d.path().join("Long.md"),
            format!("# Long\n{}", "water ".repeat(300)),
        )
        .unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let mut seen = Lengths(vec![]);
        let out = plain(&ix, "water", Some(&mut seen));
        assert_eq!(out.hits.len(), 1);
        assert_eq!(seen.0.len(), 1);
        assert!(seen.0[0] <= RERANK_CHARS + 1, "{}", seen.0[0]);
    }

    #[test]
    fn rerank_n_bounds_what_is_rescored() {
        let (_d, ix, _e) = vault_with_vectors();
        let mut table = Table([("steep", 0.9f32)].into_iter().collect());
        let cfg = SearchConfig {
            rerank_n: 1,
            ..SearchConfig::default()
        };
        let out = hybrid_hits(
            &ix,
            "water",
            None,
            Some(&mut table),
            &cfg,
            &MemoryConfig::default(),
            0,
            10,
        )
        .unwrap();
        // Only the first fused hit was scored; the second keeps its place.
        assert_eq!(out.iter().filter(|h| h.rerank.is_some()).count(), 1);
    }
}
