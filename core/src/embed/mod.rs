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
}
