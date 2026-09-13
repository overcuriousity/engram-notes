//! fastembed with `multilingual-e5-small`. Downloaded on first use, or loaded
//! from a folder for machines with no network.

use super::{Embedder, normalise};
use crate::{Error, Result};
use fastembed::{
    EmbeddingModel, InitOptionsUserDefined, Pooling, QuantizationMode, TextEmbedding,
    TextInitOptions, TokenizerFiles, UserDefinedEmbeddingModel,
};
use std::path::Path;

/// fastembed ships no quantized e5-small, so this is the full model.
pub const MODEL_ID: &str = "multilingual-e5-small";
pub const DIM: usize = 384;

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

impl FastEmbedder {
    /// Fetches the model into `cache_dir` if it is not there already.
    pub fn download(cache_dir: &Path) -> Result<FastEmbedder> {
        let options = TextInitOptions::new(EmbeddingModel::MultilingualE5Small)
            .with_cache_dir(cache_dir.to_path_buf())
            .with_show_download_progress(false);
        Ok(FastEmbedder {
            model: TextEmbedding::try_new(options).map_err(embed_error)?,
            id: MODEL_ID.to_owned(),
        })
    }

    /// A folder holding `model.onnx`, `tokenizer.json`, `config.json`,
    /// `special_tokens_map.json` and `tokenizer_config.json`.
    pub fn from_dir(dir: &Path) -> Result<FastEmbedder> {
        let model = UserDefinedEmbeddingModel {
            onnx_file: read(dir, "model.onnx")?,
            external_initializers: Default::default(),
            tokenizer_files: TokenizerFiles {
                tokenizer_file: read(dir, "tokenizer.json")?,
                config_file: read(dir, "config.json")?,
                special_tokens_map_file: read(dir, "special_tokens_map.json")?,
                tokenizer_config_file: read(dir, "tokenizer_config.json")?,
            },
            pooling: Some(Pooling::Mean),
            quantization: QuantizationMode::None,
            output_key: None,
        };
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "model".into());
        Ok(FastEmbedder {
            model: TextEmbedding::try_new_from_user_defined(model, InitOptionsUserDefined::new())
                .map_err(embed_error)?,
            id: format!("dir:{name}"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "downloads the model"]
    fn the_real_model_embeds_and_ranks() {
        let dir = tempfile::tempdir().unwrap();
        let mut e = FastEmbedder::download(dir.path()).unwrap();
        assert_eq!(e.dim(), DIM);
        let docs = e
            .embed_documents(&["rust ownership".into(), "kettle and beans".into()])
            .unwrap();
        assert_eq!(docs[0].len(), DIM);
        let q = e.embed_query("who owns the memory in rust").unwrap();
        let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
        assert!(dot(&q, &docs[0]) > dot(&q, &docs[1]));
    }
}
