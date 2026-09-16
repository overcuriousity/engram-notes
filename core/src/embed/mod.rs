//! The embedding seam. One trait, a deterministic fake for tests, and
//! fastembed behind the `fastembed` feature so tests need no model.

use crate::Result;

#[cfg(feature = "fastembed")]
pub mod fastembed;

pub trait Embedder: Send {
    /// Identifies the vectors in the index; changing it clears them.
    fn id(&self) -> String;
    fn dim(&self) -> usize;
    fn embed_documents(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    fn embed_query(&mut self, text: &str) -> Result<Vec<f32>>;
}

/// The cross-encoder seam: a query against a few documents, one score each.
pub trait Reranker: Send {
    /// Identifies the model; the status bar shows it.
    fn id(&self) -> String;
    /// One score per document, higher is more relevant, in `documents` order.
    fn score(&mut self, query: &str, documents: &[String]) -> Result<Vec<f32>>;
}

/// Scores the share of the query's words the document contains. No model,
/// same answer every run, and a test can arrange the order it wants.
pub struct FakeReranker;

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

impl Reranker for FakeReranker {
    fn id(&self) -> String {
        "fake-reranker".into()
    }

    fn score(&mut self, query: &str, documents: &[String]) -> Result<Vec<f32>> {
        let q = words(query);
        Ok(documents
            .iter()
            .map(|d| {
                if q.is_empty() {
                    return 0.0;
                }
                let have: std::collections::HashSet<String> = words(d).into_iter().collect();
                q.iter().filter(|w| have.contains(*w)).count() as f32 / q.len() as f32
            })
            .collect())
    }
}

/// A reranker that remembers how long its last run took and whether it
/// failed, so the shell can hold it to a budget without core knowing one.
pub struct TimedReranker {
    inner: Box<dyn Reranker>,
    last: Option<std::time::Duration>,
    last_error: Option<String>,
}

impl TimedReranker {
    pub fn new(inner: Box<dyn Reranker>) -> TimedReranker {
        TimedReranker {
            inner,
            last: None,
            last_error: None,
        }
    }

    /// The last run, taken: a search that scored nothing leaves no reading,
    /// and the budget must not halve twice on one measurement.
    pub fn take_last(&mut self) -> Option<std::time::Duration> {
        self.last.take()
    }

    pub fn take_last_error(&mut self) -> Option<String> {
        self.last_error.take()
    }
}

impl Reranker for TimedReranker {
    fn id(&self) -> String {
        self.inner.id()
    }

    fn score(&mut self, query: &str, documents: &[String]) -> Result<Vec<f32>> {
        let start = std::time::Instant::now();
        let out = self.inner.score(query, documents);
        self.last = Some(start.elapsed());
        self.last_error = out.as_ref().err().map(|e| e.to_string());
        out
    }
}

/// A cross-encoder's logit as a probability, so the divider reads it like a cosine.
pub fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Hashes words into a vector. No model, no network, same answer every run.
pub struct FakeEmbedder {
    dim: usize,
}

impl FakeEmbedder {
    pub fn new(dim: usize) -> FakeEmbedder {
        FakeEmbedder { dim }
    }
}

fn fnv(word: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in word.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

pub(crate) fn normalise(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v {
            *x /= norm;
        }
    }
}

impl Embedder for FakeEmbedder {
    fn id(&self) -> String {
        format!("fake-{}", self.dim)
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn embed_documents(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|t| {
                let mut v = vec![0.0f32; self.dim];
                for word in t.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
                    if word.is_empty() {
                        continue;
                    }
                    let h = fnv(word);
                    v[(h % self.dim as u64) as usize] += 1.0;
                    v[((h >> 32) % self.dim as u64) as usize] += 0.5;
                }
                normalise(&mut v);
                v
            })
            .collect())
    }

    fn embed_query(&mut self, text: &str) -> Result<Vec<f32>> {
        Ok(self.embed_documents(&[text.to_owned()])?.pop().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fake_is_deterministic_and_normalised() {
        let mut e = FakeEmbedder::new(64);
        assert_eq!(e.dim(), 64);
        assert_eq!(e.id(), "fake-64");
        let a = e.embed_documents(&["rust ownership".to_string()]).unwrap();
        let b = e.embed_documents(&["rust ownership".to_string()]).unwrap();
        assert_eq!(a, b);
        assert_eq!(a[0].len(), 64);
        let norm: f32 = a[0].iter().map(|x| x * x).sum();
        assert!((norm - 1.0).abs() < 1e-5, "norm was {norm}");
    }

    #[test]
    fn shared_words_score_higher_than_strangers() {
        let mut e = FakeEmbedder::new(64);
        let v = e
            .embed_documents(&[
                "rust ownership rules".to_string(),
                "ownership rules matter".to_string(),
                "coffee brewing kettle".to_string(),
            ])
            .unwrap();
        let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
        assert!(dot(&v[0], &v[1]) > dot(&v[0], &v[2]));
    }

    #[test]
    fn a_query_embeds_like_a_document() {
        let mut e = FakeEmbedder::new(64);
        let q = e.embed_query("rust").unwrap();
        let d = e.embed_documents(&["rust".to_string()]).unwrap();
        assert_eq!(q, d[0]);
    }

    #[test]
    fn empty_text_is_a_zero_vector_rather_than_a_nan() {
        let mut e = FakeEmbedder::new(8);
        assert_eq!(e.embed_query("").unwrap(), vec![0.0; 8]);
    }
    #[test]
    fn the_fake_reranker_scores_the_share_of_query_words_present() {
        let mut r = FakeReranker;
        assert_eq!(r.id(), "fake-reranker");
        let s = r
            .score(
                "rust ownership",
                &[
                    "rust ownership rules".to_string(),
                    "ownership alone".to_string(),
                    "coffee".to_string(),
                ],
            )
            .unwrap();
        assert_eq!(s, vec![1.0, 0.5, 0.0]);
        assert_eq!(r.score("", &["a".to_string()]).unwrap(), vec![0.0]);
    }

    #[test]
    fn the_timed_wrapper_records_the_last_run() {
        let mut t = TimedReranker::new(Box::new(FakeReranker));
        assert!(t.take_last().is_none());
        assert_eq!(t.id(), "fake-reranker");
        let s = t.score("a", &["a b".to_string()]).unwrap();
        assert_eq!(s, vec![1.0]);
        assert!(t.take_last().is_some());
        // Taken: a run is measured once, and a search that scores nothing must
        // not read the run before it.
        assert!(t.take_last().is_none());
        assert!(t.take_last_error().is_none());
    }

    #[test]
    fn the_timed_wrapper_keeps_the_error_of_a_failing_reranker() {
        struct Broken;
        impl Reranker for Broken {
            fn id(&self) -> String {
                "broken".into()
            }
            fn score(&mut self, _: &str, _: &[String]) -> Result<Vec<f32>> {
                Err(crate::Error::Embed("no".into()))
            }
        }
        let mut t = TimedReranker::new(Box::new(Broken));
        assert!(t.score("a", &["b".to_string()]).is_err());
        assert_eq!(
            t.take_last_error().as_deref(),
            Some(crate::Error::Embed("no".into()).to_string().as_str())
        );
    }

    #[test]
    fn sigmoid_maps_logits_into_the_unit_interval() {
        assert!((sigmoid(0.0) - 0.5).abs() < 1e-6);
        assert!(sigmoid(10.0) > 0.99);
        assert!(sigmoid(-10.0) < 0.01);
    }
}
