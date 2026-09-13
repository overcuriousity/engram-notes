//! Reciprocal rank fusion, and engram's cliff for the relevance divider.

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
/// The gaps are read over the similarities sorted, then carried back as "after
/// the last hit that still reaches the cut", so what is marked is always a
/// tail. A fused list is not in score order -- a hit both branches found sits
/// above one only the dense branch ranked higher -- and reading it position by
/// position would cut the good hits below that one away.
pub fn mark_past_divider(hits: &mut [Hit], factor: f32, min_share: f32) {
    let mut sorted: Vec<f32> = hits.iter().filter_map(|h| h.similarity).collect();
    sorted.sort_by(|a, b| b.total_cmp(a));
    let Some(above) = cliff(&sorted, factor, min_share) else {
        return;
    };
    let cut = sorted[above - 1];
    let from = hits
        .iter()
        .rposition(|h| h.similarity.is_some_and(|s| s >= cut))
        .map_or(0, |i| i + 1);
    for h in hits.iter_mut().skip(from) {
        h.past_divider = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::Hit;

    fn hit(path: &str, similarity: Option<f32>) -> Hit {
        Hit {
            path: path.into(),
            title: path.into(),
            snippet: String::new(),
            heading: None,
            line: 1,
            similarity,
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
        // Fusion put 0.40 above 0.78 and 0.76. Read position by position the
        // largest gap is the first one, and the two good hits below it would be
        // thrown away; read over the sorted scores the fall is before 0.39.
        let mut hits = vec![
            hit("a.md", Some(0.80)),
            hit("b.md", Some(0.40)),
            hit("c.md", Some(0.78)),
            hit("d.md", Some(0.76)),
            hit("e.md", Some(0.39)),
        ];
        mark_past_divider(&mut hits, CLIFF_FACTOR, CLIFF_MIN_SHARE);
        assert_eq!(
            hits.iter().map(|h| h.past_divider).collect::<Vec<_>>(),
            vec![false, false, false, false, true]
        );
    }

    #[test]
    fn a_hit_without_a_similarity_is_never_the_line_itself() {
        let mut hits = vec![
            hit("a.md", Some(0.80)),
            hit("b.md", None),
            hit("c.md", Some(0.78)),
            hit("d.md", Some(0.05)),
        ];
        mark_past_divider(&mut hits, CLIFF_FACTOR, CLIFF_MIN_SHARE);
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
        mark_past_divider(&mut hits, CLIFF_FACTOR, CLIFF_MIN_SHARE);
        assert!(hits.iter().all(|h| !h.past_divider));
    }
}
