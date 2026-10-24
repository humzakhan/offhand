# ADR-0005: Local embeddings for accessibility label retrieval

- Status: Accepted (v0.1 seam; v0.2 backend planned)
- Date: 2026-10-24

## Context

Task wording can differ from accessibility labels. A semantic shortlist could
reduce the candidate set for local T1 selection while retaining the tree as
the source of element identity. Accuracy and latency improvements remain
unverified; v0.1 has no enabled model path.

## Decision

Use local EmbeddingGemma-300M for the proposed v0.2 label embedder. The
[Google model card](https://huggingface.co/google/embeddinggemma-300m) describes
a text model with 768-dimensional output, reducible to 512, 256 or 128 dimensions.
Use the retrieval query prefix `task: search result | query: ` and label prefix
`title: none | text: `.

The [GGUF repository](https://huggingface.co/ggml-org/embeddinggemma-300M-GGUF/tree/main)
lists `embeddinggemma-300M-Q8_0.gguf` at about 334 MB. The manifest records this
optional artifact with `sha256 = "TBD"`; no download is enabled before its exact
revision and digest are verified. No weights or native dependencies ship here.

[Gemini Embedding 2](https://ai.google.dev/gemini-api/docs/models/gemini-embedding-2)
is a multimodal Gemini API model, not the local backend chosen here. D14 rejects
it for the offline path: only `offhand-cloud`, behind the opt-in `cloud` feature,
may make inference network calls. This ADR adds no cloud implementation.

Ranked uses:

1. Accessibility label shortlist before `Engine::choose` in v0.2.
2. Screen-result cache retrieval in v0.3, with fresh-tree revalidation before use.
3. Icon retrieval with a separate local SigLIP 2 backend in v0.3 or later;
   the text-only embedder does not embed screenshots or icons.
4. Audio embeddings are not used.

The v0.1 seam consists of `Embedder`, `NoopEmbedder`, cosine ranking, and an
optional slot on Agent, defaulting to None. `Agent::shortlist_candidates` is the
future T1 integration point; deterministic T0 never calls it. No embedder or
any embedding error preserves the entire original candidate list and order.
Ranking only narrows options: it neither executes actions nor bypasses
`offhand_policy::classify`, confirmation, or target revalidation.

## Acceptance condition

For v0.2, implement in-process `llama-cpp-2` embedding mode with Q8_0 weights.
Truncate output to 256 dimensions, then L2-normalise; report the effective size
through `dims()`. Cache label vectors per tree snapshot, invalidating on a new
snapshot and on model/prefix changes. Preserve original element indices when
passing shortlisted candidates to `Engine::choose` and mapping its result back.

Before enabling the path, benchmark shortlist recall and end-to-end selection
accuracy, cold/warm latency, and memory on both macOS and Windows. Record
before-and-after results from `bench/` per repository rule 5. No benchmark
numbers are claimed for this disabled seam.

## Consequences

- EmbeddingGemma uses the [Gemma Terms of Use](https://ai.google.dev/gemma/terms),
  not Apache-2.0. Legal review of redistribution, acceptance and download flow
  is required before making it a default download.
- Malformed embedding batches fail rather than silently ranking partial output;
  equal scores retain input order. Zero-norm cosine similarity is zero.
- No new tests are added for this slice under the workflow instruction. Build
  checks do not establish retrieval quality, native runtime support or latency.
