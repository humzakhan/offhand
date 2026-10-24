//! Local, text-only GGUF embedding backend. No network access occurs here.

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use llama_cpp_2::context::params::{LlamaContextParams, LlamaPoolingType};
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::{LlamaModel, params::LlamaModelParams};

use crate::{Embedder, Error, Result};

/// Retrieval query prefix; callers add it before calling `embed_text`.
pub use crate::EMBEDDINGGEMMA_QUERY_PREFIX as QUERY_PREFIX;
/// Retrieval document prefix; callers add it before calling `embed_text`.
pub use crate::EMBEDDINGGEMMA_DOCUMENT_PREFIX as DOCUMENT_PREFIX;

const CONTEXT: u32 = 2048;
const DIMENSIONS: usize = 256;
const MODEL_FILE: &str = "embeddinggemma-2-Q8_0.gguf";

// Process-wide initialization keeps the native backend alive for every model.
static BACKEND: OnceLock<std::result::Result<LlamaBackend, String>> = OnceLock::new();

/// Model location, matching `cargo xtask fetch-model embeddinggemma-2`.
#[must_use]
pub fn model_path() -> Option<PathBuf> {
    let directory = std::env::var_os("OFFHAND_MODELS_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::data_local_dir().map(|path| path.join("offhand").join("models")))?;
    Some(directory.join(MODEL_FILE))
}

/// Mean-pooled, L2-normalised text embeddings truncated to 256 dimensions.
///
/// Native handles stay behind the binding's safe API. A mutex serializes model
/// access, including context creation, which can mutate the native model.
#[derive(Debug)]
pub struct LlamaEmbedder {
    model: Mutex<LlamaModel>,
    backend: &'static LlamaBackend,
}

impl LlamaEmbedder {
    /// Load a local GGUF without downloading anything.
    ///
    /// The pinned binding currently predates `embeddinggemma2` support; that
    /// architecture returns a load error until the dependency is updated.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.is_file() {
            return Err(Error::Backend(format!("model file missing: {}", path.display())));
        }
        let backend = BACKEND
            .get_or_init(|| LlamaBackend::init().map_err(|error| error.to_string()))
            .as_ref()
            .map_err(|error| Error::Backend(error.clone()))?;
        // CPU is the portable default; no GPU runtime is required on Windows.
        let params = LlamaModelParams::default().with_n_gpu_layers(0);
        let model = LlamaModel::load_from_file(backend, path, &params).map_err(backend_error)?;
        if model.n_embd_out() < 256 {
            return Err(Error::Backend("model output has fewer than 256 dimensions".into()));
        }
        // Validate context allocation now so the factory can fall back on failure.
        drop(model.new_context(backend, context_params()).map_err(backend_error)?);
        Ok(Self {
            model: Mutex::new(model),
            backend,
        })
    }
}

impl Embedder for LlamaEmbedder {
    fn dims(&self) -> usize {
        DIMENSIONS
    }

    fn embed_text(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let model = self.model.lock().map_err(|error| backend_error(error.to_string()))?;
        let mut context = model
            .new_context(self.backend, context_params())
            .map_err(backend_error)?;
        let mut batch = LlamaBatch::new(CONTEXT as usize, 1);
        let mut embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            // Bound tokenizer allocation and its FFI length conversion as well as
            // the eventual token count. Reject long inputs rather than silently
            // discarding the task prefix or label suffix.
            if text.len() > 64 * 1024 {
                return Err(Error::Backend("embedding input exceeds 64 KiB".into()));
            }
            let tokens = model.vocab().tokenize(text.as_bytes(), true, false);
            if tokens.is_empty() || tokens.len() > CONTEXT as usize {
                return Err(Error::Backend("embedding input must fit 1..=2048 tokens".into()));
            }
            context.clear_kv_cache();
            batch.clear();
            // Every token participates in mean pooling, in a single microbatch.
            batch.add_sequence(&tokens, 0, true).map_err(backend_error)?;
            context.decode(&mut batch).map_err(backend_error)?;
            let pooled = context.embeddings_seq_ith(0).map_err(backend_error)?;
            embeddings.push(normalise_truncated(pooled)?);
        }
        Ok(embeddings)
    }
}

fn context_params() -> LlamaContextParams {
    LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(CONTEXT))
        .with_n_batch(CONTEXT)
        .with_n_ubatch(CONTEXT)
        .with_embeddings(true)
        .with_pooling_type(LlamaPoolingType::Mean)
}

// Each normalised component is in [-1, 1]; narrowing loses only f32 precision.
#[allow(clippy::cast_possible_truncation)]
fn normalise_truncated(pooled: &[f32]) -> Result<Vec<f32>> {
    if pooled.len() < DIMENSIONS || pooled.iter().any(|value| !value.is_finite()) {
        return Err(Error::Backend("invalid embedding output".into()));
    }
    let truncated = &pooled[..DIMENSIONS];
    let norm = truncated
        .iter()
        .map(|&value| f64::from(value).powi(2))
        .sum::<f64>()
        .sqrt();
    if norm == 0.0 || !norm.is_finite() {
        return Err(Error::Backend("zero or invalid embedding norm".into()));
    }
    Ok(truncated.iter().map(|&value| (f64::from(value) / norm) as f32).collect())
}

fn backend_error(error: impl std::fmt::Display) -> Error {
    Error::Backend(error.to_string())
}
