//! Reciprocal rank fusion, and engram's cliff for the relevance divider.

use crate::config::SearchConfig;
use crate::search::Hit;
use std::collections::HashMap;

/// engram's `k`. Large enough that rank 1 and rank 2 are not far apart.
pub const RRF_K: f64 = 60.0;
/// The fall must be this many times the mean of the other gaps.
pub const CLIFF_FACTOR: f32 = 3.0;
/// ...and at least this share of the top score, so a flat list has no fall.
pub const CLIFF_MIN_SHARE: f32 = 0.01;

/// One entry per key, scored `sum of 1/(k + rank)` over the branches, best first.
pub fn rrf(branches: &[Vec<String>], k: f64) -> Vec<(String, f64)> {
    let mut score: HashMap<&str, f64> = HashMap::new();
    let mut first: HashMap<&str, usize> = HashMap::new();
    for branch in branches {
        for (rank, key) in branch.iter().enumerate() {
            *score.entry(key).or_insert(0.0) += 1.0 / (k + rank as f64 + 1.0);
            first.entry(key).or_insert(rank);
        }
    }
    let mut out: Vec<(String, f64)> = score.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
    // The best rank a branch gave it breaks ties, then the path, so the order
    // is the same on every run.
    out.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then(first[a.0.as_str()].cmp(&first[b.0.as_str()]))
            .then(a.0.cmp(&b.0))
    });
    out
}

/// Where a descending list of scores falls off, if it does.
///
/// engram's rule: at least three scores, the largest gap larger than
/// `factor` times the mean of the others and larger than `min_share` of the top.
pub fn cliff(scores: &[f32], factor: f32, min_share: f32) -> Option<usize> {
    if scores.len() < 3 {
        return None;
    }
    let gaps: Vec<f32> = scores.windows(2).map(|w| w[0] - w[1]).collect();
    let (at, largest) = gaps
        .iter()
        .copied()
        .enumerate()
        .fold(
            (0usize, 0.0f32),
            |best, (i, g)| {
                if g > best.1 { (i, g) } else { best }
            },
        );
    if largest <= 0.0 || largest <= min_share * scores[0].abs() {
        return None;
    }
    let others = (gaps.iter().sum::<f32>() - largest) / (gaps.len() - 1) as f32;
    (largest > factor * others).then_some(at + 1)
}

/// Flag every hit from the fall on, leaving the list in its order.
///
/// Reads the rerank score where reranking ran and the cosine otherwise, each
/// against its own floor. The gaps are read over the scores sorted, then
/// carried back as "after the last hit that still reaches the cut", so what
/// is marked is always a tail. A fused list is not in score order -- a hit
/// both branches found sits above one only the dense branch ranked higher --
/// and reading it position by position would cut the good hits below that
/// one away.
///
/// Reranking scores the head of the list and not the rest, so the two readings
/// meet here: the cliff is read over one scale, whichever one the search used,
/// while the floor asks each hit for the score it actually has.
pub fn mark_past_divider(hits: &mut [Hit], cfg: &SearchConfig) {
    let reranked = hits.iter().any(|h| h.rerank.is_some());
    let read: fn(&Hit) -> Option<f32> = match reranked {
        true => |h| h.rerank,
        false => |h| h.similarity,
    };
    let mut sorted: Vec<f32> = hits.iter().filter_map(read).collect();
    if sorted.is_empty() {
        return;
    }
    sorted.sort_by(|a, b| b.total_cmp(a));
    // The later of two readings, so whichever leaves more hits standing wins:
    // the cliff finds a fall within the list, the floor knows that a score can
    // be the best one here and still mean nothing.
    let by_cliff = cliff(&sorted, cfg.cliff_factor, cfg.cliff_min_share)
        .map(|above| tail_from(hits, read, sorted[above - 1]));
    let by_floor = floor_tail(hits, cfg);
    let from = by_cliff.unwrap_or(0).max(by_floor);
    for h in hits.iter_mut().skip(from) {
        h.past_divider = true;
    }
}

// The place after the last hit that reaches the floor of its own scale. Only
// the first `rerank_n` hits are rescored, so most of a long list holds nothing
// but a cosine: read for a rerank score it never had, every one of them would
// fall, however close it is.
fn floor_tail(hits: &[Hit], cfg: &SearchConfig) -> usize {
    hits.iter()
        .rposition(|h| match h.rerank {
            Some(s) => s >= cfg.rerank_floor,
            None => h.similarity.is_some_and(|s| s >= cfg.similarity_floor),
        })
        .map_or(0, |i| i + 1)
}

// The place after the last hit that still reaches `cut`, so what is marked is
// always a tail.
fn tail_from(hits: &[Hit], read: fn(&Hit) -> Option<f32>, cut: f32) -> usize {
    hits.iter()
        .rposition(|h| read(h).is_some_and(|s| s >= cut))
        .map_or(0, |i| i + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SearchConfig;
    use crate::search::Hit;

    fn hit(path: &str, similarity: Option<f32>) -> Hit {
        Hit {
            path: path.into(),
            title: path.into(),
            snippet: String::new(),
            line: 1,
            similarity,
            rerank: None,
            score: 0.0,
            past_divider: false,
            primed: false,
        }
    }

    #[test]
    fn fusion_rewards_a_note_both_branches_found() {
        let dense = vec!["a.md".to_string(), "b.md".to_string()];
        let sparse = vec!["c.md".to_string(), "a.md".to_string()];
        let fused = rrf(&[dense, sparse], RRF_K);
        assert_eq!(fused[0].0, "a.md");
        assert_eq!(fused.len(), 3);
        assert!(fused[0].1 > fused[1].1);
        assert!(fused.windows(2).all(|w| w[0].1 >= w[1].1));
    }

    #[test]
    fn the_cliff_needs_three_scores_and_a_gap_that_stands_out() {
        assert_eq!(cliff(&[0.9, 0.2], CLIFF_FACTOR, CLIFF_MIN_SHARE), None);
        assert_eq!(
            cliff(&[0.90, 0.88, 0.86], CLIFF_FACTOR, CLIFF_MIN_SHARE),
            None
        );
        assert_eq!(
            cliff(&[0.90, 0.88, 0.30, 0.29], CLIFF_FACTOR, CLIFF_MIN_SHARE),
            Some(2)
        );
        // A tie is not a fall.
        assert_eq!(cliff(&[0.5, 0.5, 0.5], CLIFF_FACTOR, CLIFF_MIN_SHARE), None);
    }

    #[test]
    fn marking_leaves_a_tail_even_when_the_list_is_out_of_order() {
        // Fusion put 0.40 above 0.87 and 0.86. Read position by position the
        // largest gap is the first one, and the two good hits below it would be
        // thrown away; read over the sorted scores the fall is before 0.39.
        let mut hits = vec![
            hit("a.md", Some(0.88)),
            hit("b.md", Some(0.40)),
            hit("c.md", Some(0.87)),
            hit("d.md", Some(0.86)),
            hit("e.md", Some(0.39)),
        ];
        mark_past_divider(&mut hits, &SearchConfig::default());
        assert_eq!(
            hits.iter().map(|h| h.past_divider).collect::<Vec<_>>(),
            vec![false, false, false, false, true]
        );
    }

    #[test]
    fn a_hit_without_a_similarity_is_never_the_line_itself() {
        let mut hits = vec![
            hit("a.md", Some(0.88)),
            hit("b.md", None),
            hit("c.md", Some(0.86)),
            hit("d.md", Some(0.05)),
        ];
        mark_past_divider(&mut hits, &SearchConfig::default());
        assert!(!hits[1].past_divider);
        assert!(hits[3].past_divider);
    }

    #[test]
    fn nothing_is_marked_when_there_is_no_fall() {
        let mut hits = vec![
            hit("a.md", Some(0.9)),
            hit("b.md", Some(0.88)),
            hit("c.md", Some(0.86)),
        ];
        mark_past_divider(&mut hits, &SearchConfig::default());
        assert!(hits.iter().all(|h| !h.past_divider));
    }

    #[test]
    fn the_floor_draws_the_line_where_the_cliff_finds_no_fall() {
        // Evenly spaced, so the cliff says nothing; the last two are strangers.
        let mut hits = vec![
            hit("a.md", Some(0.87)),
            hit("b.md", Some(0.85)),
            hit("c.md", Some(0.84)),
            hit("d.md", Some(0.79)),
            hit("e.md", Some(0.77)),
        ];
        mark_past_divider(&mut hits, &SearchConfig::default());
        assert_eq!(
            hits.iter().map(|h| h.past_divider).collect::<Vec<_>>(),
            vec![false, false, false, true, true]
        );
    }

    #[test]
    fn the_line_is_wherever_leaves_more_hits_standing() {
        // What the vault showed: the cliff cut after the standout and called
        // three real neighbours loose. The floor keeps them.
        let mut hits = vec![
            hit("a.md", Some(0.95)),
            hit("b.md", Some(0.86)),
            hit("c.md", Some(0.855)),
            hit("d.md", Some(0.85)),
        ];
        mark_past_divider(&mut hits, &SearchConfig::default());
        assert!(hits.iter().all(|h| !h.past_divider));
    }

    #[test]
    fn a_whole_list_of_strangers_is_all_loose() {
        let mut hits = vec![
            hit("a.md", Some(0.78)),
            hit("b.md", Some(0.77)),
            hit("c.md", Some(0.76)),
        ];
        mark_past_divider(&mut hits, &SearchConfig::default());
        assert!(hits.iter().all(|h| h.past_divider));
    }
    #[test]
    fn the_divider_reads_rerank_scores_when_any_hit_has_one() {
        let mut hits = vec![
            hit("a", Some(0.9)),
            hit("b", Some(0.9)),
            hit("c", Some(0.9)),
        ];
        hits[0].rerank = Some(0.95);
        hits[1].rerank = Some(0.90);
        hits[2].rerank = Some(0.02);
        mark_past_divider(&mut hits, &SearchConfig::default());
        assert!(!hits[0].past_divider && !hits[1].past_divider);
        assert!(hits[2].past_divider, "0.02 is under rerank_floor 0.1");
    }

    #[test]
    fn a_hit_the_reranker_never_reached_is_read_by_its_cosine() {
        // What a default search does: `limit` 50, `rerank_n` 20, so the tail
        // holds a cosine and no rerank score. Read against the rerank floor it
        // would fall whole; each hit is read against its own floor instead.
        let mut hits: Vec<Hit> = (0..6).map(|i| hit(&format!("{i}.md"), None)).collect();
        for (i, h) in hits.iter_mut().enumerate() {
            match i < 3 {
                true => h.rerank = Some(0.9 - i as f32 * 0.01),
                false => h.similarity = Some(0.9 - i as f32 * 0.01),
            }
        }
        mark_past_divider(&mut hits, &SearchConfig::default());
        assert!(
            hits.iter().all(|h| !h.past_divider),
            "0.87 is over similarity_floor, scored or not"
        );

        // ...and the tail still falls on its own floor when it earns it.
        hits[5].similarity = Some(0.10);
        hits[5].past_divider = false;
        mark_past_divider(&mut hits, &SearchConfig::default());
        assert!(hits[5].past_divider);
        assert!(!hits[4].past_divider);
    }
}
