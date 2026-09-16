//! fastembed over folders of ONNX files: the bundled models, or a folder the
//! writer points at. Nothing here opens a connection.

use super::{Embedder, Reranker, normalise, sigmoid};
use crate::{Error, Result};
use fastembed::{
    InitOptionsUserDefined, Pooling, QuantizationMode, RerankInitOptionsUserDefined, TextEmbedding,
    TextRerank, TokenizerFiles, UserDefinedEmbeddingModel, UserDefinedRerankingModel,
};
use std::path::Path;

/// `multilingual-e5-small`, dynamically quantised to int8.
pub const EMBEDDER_ID: &str = "multilingual-e5-small-int8";
/// `cross-encoder/mmarco-mMiniLMv2-L12-H384-v1`, dynamically quantised to int8.
pub const RERANKER_ID: &str = "mmarco-mMiniLMv2-L12-H384-v1-int8";
pub const DIM: usize = 384;
/// Both models take 512 tokens.
const MAX_LENGTH: usize = 512;

pub struct FastEmbedder {
    model: TextEmbedding,
    id: String,
}

fn embed_error(e: impl std::fmt::Display) -> Error {
    Error::Embed(e.to_string())
}

fn read(dir: &Path, name: &str) -> Result<Vec<u8>> {
    std::fs::read(dir.join(name)).map_err(|e| Error::io(dir.join(name), e))
}

/// The four tokenizer files every model folder holds beside `model.onnx`.
fn tokenizer_files(dir: &Path) -> Result<TokenizerFiles> {
    Ok(TokenizerFiles {
        tokenizer_file: read(dir, "tokenizer.json")?,
        config_file: read(dir, "config.json")?,
        special_tokens_map_file: read(dir, "special_tokens_map.json")?,
        tokenizer_config_file: read(dir, "tokenizer_config.json")?,
    })
}

/// The id an override folder gets, so the index knows its vectors are not the
/// bundled model's.
pub fn dir_id(dir: &Path) -> String {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "model".into());
    format!("dir:{name}")
}

impl FastEmbedder {
    /// A folder holding `model.onnx`, `tokenizer.json`, `config.json`,
    /// `special_tokens_map.json` and `tokenizer_config.json`.
    pub fn from_dir(dir: &Path, id: &str) -> Result<FastEmbedder> {
        let model = UserDefinedEmbeddingModel {
            onnx_file: read(dir, "model.onnx")?,
            external_initializers: Default::default(),
            tokenizer_files: tokenizer_files(dir)?,
            pooling: Some(Pooling::Mean),
            quantization: QuantizationMode::None,
            output_key: None,
        };
        Ok(FastEmbedder {
            model: TextEmbedding::try_new_from_user_defined(model, InitOptionsUserDefined::new())
                .map_err(embed_error)?,
            id: id.to_owned(),
        })
    }
}

impl Embedder for FastEmbedder {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn dim(&self) -> usize {
        DIM
    }

    // e5 distinguishes a question from a document; fastembed adds no prefix.
    fn embed_documents(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let prefixed: Vec<String> = texts.iter().map(|t| format!("passage: {t}")).collect();
        let mut out = self.model.embed(prefixed, None).map_err(embed_error)?;
        for v in &mut out {
            normalise(v);
        }
        Ok(out)
    }

    fn embed_query(&mut self, text: &str) -> Result<Vec<f32>> {
        let mut out = self
            .model
            .embed(vec![format!("query: {text}")], None)
            .map_err(embed_error)?;
        let mut v = out.pop().ok_or_else(|| Error::Embed("no vector".into()))?;
        normalise(&mut v);
        Ok(v)
    }
}

pub struct FastReranker {
    model: TextRerank,
    id: String,
}

impl FastReranker {
    /// The same five files as the embedder's folder.
    pub fn from_dir(dir: &Path, id: &str) -> Result<FastReranker> {
        let model = UserDefinedRerankingModel::new(read(dir, "model.onnx")?, tokenizer_files(dir)?);
        let options = RerankInitOptionsUserDefined::new().with_max_length(MAX_LENGTH);
        Ok(FastReranker {
            model: TextRerank::try_new_from_user_defined(model, options).map_err(embed_error)?,
            id: id.to_owned(),
        })
    }
}

impl Reranker for FastReranker {
    fn id(&self) -> String {
        self.id.clone()
    }

    // fastembed returns the pairs sorted by score; the trait promises document order.
    fn score(&mut self, query: &str, documents: &[String]) -> Result<Vec<f32>> {
        if documents.is_empty() {
            return Ok(vec![]);
        }
        let docs: Vec<&str> = documents.iter().map(String::as_str).collect();
        let ranked = self
            .model
            .rerank(query, docs.as_slice(), false, None)
            .map_err(embed_error)?;
        let mut out = vec![0.0f32; documents.len()];
        for r in ranked {
            let slot = out
                .get_mut(r.index)
                .ok_or_else(|| Error::Embed(format!("rerank index {} out of range", r.index)))?;
            *slot = sigmoid(r.score);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Both need the folders `scripts/fetch-models.sh` fills.
    fn models() -> std::path::PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src-tauri/models");
        assert!(
            dir.join("embedder/model.onnx").exists(),
            "run scripts/fetch-models.sh first"
        );
        dir
    }

    #[test]
    #[ignore = "needs src-tauri/models from scripts/fetch-models.sh"]
    fn the_bundled_embedder_embeds_and_ranks() {
        let mut e = FastEmbedder::from_dir(&models().join("embedder"), EMBEDDER_ID).unwrap();
        assert_eq!(e.dim(), DIM);
        assert_eq!(e.id(), EMBEDDER_ID);
        let docs = e
            .embed_documents(&["rust ownership".into(), "kettle and beans".into()])
            .unwrap();
        assert_eq!(docs[0].len(), DIM);
        let q = e.embed_query("who owns the memory in rust").unwrap();
        let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
        assert!(dot(&q, &docs[0]) > dot(&q, &docs[1]));
    }

    #[test]
    #[ignore = "needs src-tauri/models from scripts/fetch-models.sh"]
    fn the_bundled_reranker_scores_in_document_order() {
        let mut r = FastReranker::from_dir(&models().join("reranker"), RERANKER_ID).unwrap();
        let s = r
            .score(
                "who owns the memory in rust",
                &["kettle and beans".into(), "rust ownership rules".into()],
            )
            .unwrap();
        assert_eq!(s.len(), 2);
        assert!(s[1] > s[0], "{s:?}");
        assert!(s.iter().all(|x| (0.0..=1.0).contains(x)));
    }

    // The budget's default was set from this number; run it on a new machine
    // with `-- --ignored --nocapture` before changing the default.
    #[test]
    #[ignore = "needs src-tauri/models from scripts/fetch-models.sh"]
    fn one_search_of_pairs_is_timed() {
        let mut r = FastReranker::from_dir(&models().join("reranker"), RERANKER_ID).unwrap();
        let passage =
            "the vault keeps a folder of markdown notes and an index that can be rebuilt "
                .repeat(16);
        r.score("warm up", &vec![passage.clone(); 4]).unwrap();
        for (n, chars) in [
            (20, 1200),
            (20, 600),
            (20, 300),
            (10, 1200),
            (10, 600),
            (10, 300),
        ] {
            let docs = vec![passage[..chars].to_string(); n];
            let start = std::time::Instant::now();
            r.score("a query about the notes", &docs).unwrap();
            eprintln!("{n} pairs of {chars} chars: {:?}", start.elapsed());
        }
    }

    #[test]
    fn an_override_folder_is_named_after_itself() {
        assert_eq!(dir_id(Path::new("/x/my-model")), "dir:my-model");
    }
}
