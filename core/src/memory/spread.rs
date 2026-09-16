//! Notes the embedding calls strangers and the person calls inseparable.

use crate::Result;
use crate::config::MemoryConfig;
use crate::index::Index;
use crate::search::{Associated, Hit};
use std::collections::HashSet;

/// The top three hits are anchors: their strongest links that are not already
/// in the list, strongest first, at most `spread_max`. One hop, no reordering.
pub fn spread(index: &Index, hits: &[Hit], cfg: &MemoryConfig, at: i64) -> Result<Vec<Associated>> {
    if cfg.spread_max == 0 || hits.is_empty() {
        return Ok(vec![]);
    }
    let anchors: Vec<String> = hits.iter().take(3).map(|h| h.path.clone()).collect();
    let listed: HashSet<&str> = hits.iter().map(|h| h.path.as_str()).collect();
    let titles: std::collections::HashMap<String, String> = index.titles()?.into_iter().collect();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for row in index.assoc_from(&anchors, at, cfg)? {
        if listed.contains(row.other.as_str()) || !seen.insert(row.other.clone()) {
            continue;
        }
        out.push(Associated {
            title: titles
                .get(&row.other)
                .cloned()
                .unwrap_or_else(|| row.other.clone()),
            path: row.other,
            via: row.via,
            cue: row.cue,
            strength: row.value,
        });
        if out.len() >= cfg.spread_max {
            break;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MemoryConfig;
    use crate::index::Index;
    use crate::search::Hit;
    use crate::vault::Vault;
    use std::fs;

    fn hit(path: &str) -> Hit {
        Hit {
            path: path.into(),
            title: path.into(),
            snippet: String::new(),
            line: 1,
            similarity: None,
            rerank: None,
            score: 0.0,
            past_divider: false,
            primed: false,
        }
    }

    fn ix() -> (tempfile::TempDir, Index) {
        let d = tempfile::tempdir().unwrap();
        for name in ["A", "B", "C", "D", "E"] {
            fs::write(d.path().join(format!("{name}.md")), "body").unwrap();
        }
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        (d, ix)
    }

    // Two co-appearances put a link above the show threshold of 2.0.
    fn bind(ix: &mut Index, cfg: &MemoryConfig, a: &str, b: &str, cue: Option<&str>) {
        for _ in 0..2 {
            ix.bump_assoc(a, b, 1.0, cue, cfg, 0).unwrap();
        }
    }

    #[test]
    fn a_note_linked_to_a_top_hit_is_offered_with_its_cue() {
        let (_d, mut ix) = ix();
        let cfg = MemoryConfig::default();
        bind(&mut ix, &cfg, "A.md", "D.md", Some("rust"));
        let out = spread(&ix, &[hit("A.md"), hit("B.md")], &cfg, 0).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "D.md");
        assert_eq!(out[0].via, "A.md");
        assert_eq!(out[0].cue.as_deref(), Some("rust"));
    }

    #[test]
    fn a_note_already_in_the_list_is_not_offered_again() {
        let (_d, mut ix) = ix();
        let cfg = MemoryConfig::default();
        bind(&mut ix, &cfg, "A.md", "B.md", None);
        assert!(
            spread(&ix, &[hit("A.md"), hit("B.md")], &cfg, 0)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn only_the_top_three_hits_are_anchors_and_the_spread_is_capped() {
        let (_d, mut ix) = ix();
        let cfg = MemoryConfig::default();
        bind(&mut ix, &cfg, "E.md", "D.md", None); // E is the fourth hit
        let hits = [hit("A.md"), hit("B.md"), hit("C.md"), hit("E.md")];
        assert!(spread(&ix, &hits, &cfg, 0).unwrap().is_empty());
    }

    #[test]
    fn a_weak_link_stays_below_the_line_and_decay_takes_it_there() {
        let (_d, mut ix) = ix();
        let cfg = MemoryConfig::default();
        ix.bump_assoc("A.md", "D.md", 1.0, None, &cfg, 0).unwrap();
        assert!(spread(&ix, &[hit("A.md")], &cfg, 0).unwrap().is_empty());
        bind(&mut ix, &cfg, "A.md", "D.md", None);
        assert_eq!(spread(&ix, &[hit("A.md")], &cfg, 0).unwrap().len(), 1);
        // Three half-lives later the same link is 0.375 and shows no more.
        assert!(
            spread(&ix, &[hit("A.md")], &cfg, 270 * 86_400)
                .unwrap()
                .is_empty()
        );
    }
}
