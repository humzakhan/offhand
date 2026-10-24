//! Inference engine interface.
//!
//! The `llama-cpp-2` backend lands after the week-1 spike (ADR-0002); until then this crate
//! defines the [`Engine`] and [`Embedder`] seams without a native runtime.

use offhand_platform::{Element, Frame};

#[cfg(feature = "embed-llama")]
pub mod llama_embed;

/// Result alias for inference operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Inference errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No backend is available yet.
    #[error("not implemented")]
    Unimplemented,
    /// The backend failed.
    #[error("inference failed: {0}")]
    Backend(String),
}

/// The option the model picked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Choice {
    /// Index into the `options` slice passed to [`Engine::choose`].
    pub index: usize,
    /// Probability assigned to that option, in `0.0..=1.0`.
    pub prob: f64,
}

/// A local vision-language model backend.
pub trait Engine {
    /// Pick the best element for `task` from `options`, given the current screen.
    fn choose(&self, frame: &Frame, task: &str, options: &[Element]) -> Result<Choice>;
    /// Locate the screen point to act on for `task` when the accessibility tree is not enough.
    fn ground(&self, frame: &Frame, task: &str) -> Result<(f64, f64)>;
}

/// Deliberately unavailable inference backend for tree-only runs.
#[derive(Debug, Default)]
pub struct NoopEngine;
impl Engine for NoopEngine {
    fn choose(&self, _frame: &Frame, _task: &str, _options: &[Element]) -> Result<Choice> {
        Err(Error::Unimplemented)
    }
    fn ground(&self, _frame: &Frame, _task: &str) -> Result<(f64, f64)> {
        Err(Error::Unimplemented)
    }
}

/// `EmbeddingGemma` retrieval prefix for a task phrase.
pub const EMBEDDINGGEMMA_QUERY_PREFIX: &str = "task: search result | query: ";

/// `EmbeddingGemma` retrieval prefix for an accessibility label without a title.
pub const EMBEDDINGGEMMA_DOCUMENT_PREFIX: &str = "title: none | text: ";

/// A local embedding backend; implementations must not open network connections.
///
/// Text and image vectors from one backend share a space and effective dimension,
/// so [`cosine`] compares them directly. Image support is optional.
pub trait Embedder: std::fmt::Debug + Send + Sync {
    /// Number of components returned for each text, after any truncation.
    fn dims(&self) -> usize;

    /// Embed already-prefixed texts in order, returning one finite vector per text.
    ///
    /// Each vector must have [`Self::dims`] components. Return an error when the
    /// backend is unavailable; callers retain their original candidates on failure.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unimplemented`] without a backend, or [`Error::Backend`]
    /// when local inference fails.
    fn embed_text(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;

    /// Embed RGBA frames or element crops into the same space as text vectors.
    ///
    /// Returns one vector per frame in input order, each with [`Self::dims`]
    /// finite components. Images need no text prefix. Text-only backends return
    /// [`Error::Unimplemented`] by default.
    fn embed_image(&self, _images: &[Frame]) -> Result<Vec<Vec<f32>>> {
        Err(Error::Unimplemented)
    }
}

/// Load the installed local embedder when a backend is available.
///
/// The seam-only build has no backend and returns None. Backend integration can
/// replace this body behind its feature without changing runner construction.
#[must_use]
pub fn default_embedder() -> Option<Box<dyn Embedder>> {
    #[cfg(feature = "embed-llama")]
    {
        let path = llama_embed::model_path()?;
        if path.is_file() {
            match llama_embed::LlamaEmbedder::load(&path) {
                Ok(embedder) => return Some(Box::new(embedder)),
                Err(error) => eprintln!("Offhand: local embedder unavailable: {error}"),
            }
        }
    }
    None
}

/// Unavailable embedding backend for the v0.1 seam; no weights are loaded.
#[derive(Debug, Default)]
pub struct NoopEmbedder;

impl Embedder for NoopEmbedder {
    fn dims(&self) -> usize {
        0
    }

    fn embed_text(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        Err(Error::Unimplemented)
    }
}

/// Cosine similarity, or zero for mismatched lengths, zero norms or non-finite input.
///
/// Accumulation uses f64 to avoid overflow for finite f32 components.
/// Text and image embeddings can be compared when produced in the same space.
#[must_use]
// The cosine is clamped to [-1, 1]; narrowing only loses expected f32 precision.
#[allow(clippy::cast_possible_truncation)]
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let (mut dot, mut norm_a, mut norm_b) = (0.0_f64, 0.0_f64, 0.0_f64);
    for (&a, &b) in a.iter().zip(b) {
        if !a.is_finite() || !b.is_finite() {
            return 0.0;
        }
        let (a, b) = (f64::from(a), f64::from(b));
        dot += a * b;
        norm_a += a * a;
        norm_b += b * b;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    (dot / (norm_a.sqrt() * norm_b.sqrt())).clamp(-1.0, 1.0) as f32
}

/// Rank label indices by descending cosine similarity to a task phrase.
///
/// Adds the `EmbeddingGemma` retrieval prefixes and embeds the query followed by
/// the labels in one batch. Returns at most `k` indices; ties keep input order.
/// Empty labels or zero `k` return an empty list without invoking the backend.
/// Backend errors and malformed batches return an error, never a partial ranking.
/// This only ranks candidates; it does not select or authorize an action.
///
/// # Errors
///
/// Propagates embedding errors. Returns [`Error::Backend`] for a zero dimension,
/// wrong vector count or dimensions, or non-finite components.
pub fn shortlist(
    embedder: &dyn Embedder,
    query: &str,
    labels: &[&str],
    k: usize,
) -> Result<Vec<usize>> {
    Ok(shortlist_with_scores(embedder, query, labels, k)?
        .into_iter()
        .map(|(index, _)| index)
        .collect())
}

/// Rank label indices with their cosine scores, using [`shortlist`]'s batch contract.
///
/// Returns at most `k` pairs in descending score order, with input order breaking
/// ties. Scores let callers reject weak semantic matches without embedding twice.
/// Backend errors and malformed batches return an error, never a partial ranking.
///
/// # Errors
///
/// Propagates embedding errors. Returns [`Error::Backend`] for a zero dimension,
/// wrong vector count or dimensions, or non-finite components.
pub fn shortlist_with_scores(
    embedder: &dyn Embedder,
    query: &str,
    labels: &[&str],
    k: usize,
) -> Result<Vec<(usize, f32)>> {
    if labels.is_empty() || k == 0 {
        return Ok(Vec::new());
    }
    let texts: Vec<String> = std::iter::once(format!("{EMBEDDINGGEMMA_QUERY_PREFIX}{query}"))
        .chain(
            labels
                .iter()
                .map(|label| format!("{EMBEDDINGGEMMA_DOCUMENT_PREFIX}{label}")),
        )
        .collect();
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let vectors = embedder.embed_text(&refs)?;
    let dims = embedder.dims();
    if dims == 0
        || vectors.len() != texts.len()
        || vectors
            .iter()
            .any(|vector| vector.len() != dims || vector.iter().any(|value| !value.is_finite()))
    {
        return Err(Error::Backend(
            "invalid embedding batch shape or values".into(),
        ));
    }
    let mut scores: Vec<_> = vectors[1..]
        .iter()
        .enumerate()
        .map(|(index, vector)| (index, cosine(&vectors[0], vector)))
        .collect();
    scores.sort_by(|(a_index, a), (b_index, b)| b.total_cmp(a).then_with(|| a_index.cmp(b_index)));
    scores.truncate(k);
    Ok(scores)
}
