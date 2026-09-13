//! engram's bounded lift: a hit the user reaches for climbs, a little.

use crate::search::Hit;
use std::collections::HashMap;

/// Move hits up on activation, within hard bounds.
///
/// Rank-based, not score-based: a fused score means nothing across queries,
/// while "moved up two places" means the same thing every time. Activation is
/// normalised within this one list, so `margin` is a fraction of the most
/// accessible hit here. Index 0 is untouchable and index 1 cannot move, because
/// moving it would displace rank 1 -- an exact match is never buried.
///
/// Every destination is decided against the original order in one pass, then
/// one stable sort reorders the list; deciding as the list moves would let a
/// row borrow the gap another row's move opened.
pub fn prime(hits: &mut Vec<Hit>, activation: &HashMap<String, f64>, margin: f64, lift: usize) {
    let n = hits.len();
    if lift == 0 || n < 3 {
        return;
    }
    let raw: Vec<f64> = hits
        .iter()
        .map(|h| activation.get(&h.path).copied().unwrap_or(0.0))
        .collect();
    let max = raw.iter().copied().fold(0.0f64, f64::max);
    if max <= 0.0 {
        return;
    }
    // The spec's extra rule: only above the list's own median may climb.
    let mut sorted = raw.clone();
    sorted.sort_by(f64::total_cmp);
    let median = match n % 2 {
        0 => (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0,
        _ => sorted[n / 2],
    };
    let acts: Vec<f64> = raw.iter().map(|a| a / max).collect();

    let mut climb = vec![0usize; n];
    for i in 2..n {
        if raw[i] <= median {
            continue;
        }
        let mut c = 0usize;
        while c < lift {
            let predecessor = i - c - 1;
            if predecessor < 1 || acts[i] - acts[predecessor] <= margin {
                break;
            }
            c += 1;
        }
        climb[i] = c;
    }

    // The secondary key is load-bearing: without it a climbing row sorts behind
    // the row it was meant to pass, its original index being larger.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (i - climb[i], u8::from(climb[i] == 0), i));
    let mut slots: Vec<Option<Hit>> = hits.drain(..).map(Some).collect();
    for i in order {
        let mut h = slots[i].take().expect("each hit is moved once");
        h.primed = climb[i] > 0;
        hits.push(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::Hit;
    use std::collections::HashMap;

    fn hits(paths: &[&str]) -> Vec<Hit> {
        paths
            .iter()
            .map(|p| Hit {
                path: (*p).into(),
                title: (*p).into(),
                snippet: String::new(),
                heading: None,
                line: 1,
                similarity: None,
                score: 0.0,
                past_divider: false,
                primed: false,
            })
            .collect()
    }

    fn paths(h: &[Hit]) -> Vec<&str> {
        h.iter().map(|x| x.path.as_str()).collect()
    }

    #[test]
    fn an_activated_hit_climbs_at_most_the_lift() {
        let mut h = hits(&["a", "b", "c", "d", "e"]);
        let act = HashMap::from([("e".to_string(), 10.0)]);
        prime(&mut h, &act, 0.5, 2);
        assert_eq!(paths(&h), vec!["a", "b", "e", "c", "d"]);
        assert!(h[2].primed);
        assert!(!h[0].primed);
    }

    #[test]
    fn the_first_two_places_never_move() {
        let mut h = hits(&["a", "b", "c"]);
        let act = HashMap::from([("c".to_string(), 10.0)]);
        prime(&mut h, &act, 0.5, 2);
        assert_eq!(paths(&h), vec!["a", "c", "b"]);
    }

    #[test]
    fn only_hits_above_the_median_may_climb() {
        // Median of [0, 0, 0, 0, 10] is 0, so only "e" is above it.
        let mut h = hits(&["a", "b", "c", "d", "e"]);
        let act = HashMap::from([("d".to_string(), 0.0), ("e".to_string(), 10.0)]);
        prime(&mut h, &act, 0.5, 2);
        assert_eq!(paths(&h), vec!["a", "b", "e", "c", "d"]);
    }

    #[test]
    fn no_activation_leaves_the_list_alone() {
        let mut h = hits(&["a", "b", "c", "d"]);
        prime(&mut h, &HashMap::new(), 0.5, 2);
        assert_eq!(paths(&h), vec!["a", "b", "c", "d"]);
        assert!(h.iter().all(|x| !x.primed));
    }

    #[test]
    fn a_short_list_is_a_list_nothing_moves_in() {
        let mut h = hits(&["a", "b"]);
        let act = HashMap::from([("b".to_string(), 10.0)]);
        prime(&mut h, &act, 0.5, 2);
        assert_eq!(paths(&h), vec!["a", "b"]);
    }
}
