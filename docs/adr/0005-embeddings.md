# ADR-0005: Local multimodal embeddings

- Status: Accepted
- Date: 2026-10-24

## Context

Task wording can differ from accessibility labels. Semantic retrieval can rank
those labels while retaining the tree as the source of element identity.
Unlabeled icons may also benefit from matching element crops against text
queries before a VLM call. Accuracy and latency improvements need measurement.

## Decision

D14b selects **EmbeddingGemma 2**, an open-weights local multimodal model under
Apache-2.0, replacing the initial EmbeddingGemma-300M proposal. The
[official documentation](https://ai.google.dev/gemma/docs/embeddinggemma),
updated 2026-10-06, distinguishes it from the earlier Gemma-licensed model.

The [model card](https://huggingface.co/google/embeddinggemma-2) describes 740M
parameters: 270M for text, 170M for vision and 300M for audio. Text, images,
video and audio map into a shared 768-dimensional space, supporting Matryoshka
truncation to 512, 256 or 128 dimensions. Selective text-only loading is
documented. D14b's approximately 191 MB text-only footprint is not the selected
Q8_0 artifact's size.

D20 resolves the runtime to `llama-cpp-2`, consistent with ADR-0002, using
llama.cpp at or after `4fbc76d` from merged
[PR #30054](https://github.com/ggml-org/llama.cpp/pull/30054). The selected
[text-tower GGUF](https://huggingface.co/unsloth/embeddinggemma-2-GGUF/tree/main)
is `unsloth/embeddinggemma-2-GGUF/embeddinggemma-2-Q8_0.gguf`, about 310 MB.
The manifest leaves SHA-256 TBD until the exact artifact is verified. Vision
through mtmd remains unverified. No ONNX/ort runtime is introduced; MLX alone
would break macOS/Windows parity.

Ranked uses:

1. Text: semantic ranking of task phrase versus accessibility labels, reusable
   as a shortlist before `Engine::choose` in v0.2.
2. Image: unlabeled-icon and element-crop matching against text queries in the
   shared space, helping T2 before any VLM call in v0.3.
3. Screen-state cache and recognition in v0.3, with fresh-tree revalidation.
4. Audio: not used.

[Gemini Embedding 2](https://ai.google.dev/gemini-api/docs/models/gemini-embedding-2)
is a separate cloud-only API model and is rejected for the offline path. At
most it could be an optional future `offhand-cloud` backend behind `cloud`;
this ADR adds no network implementation.

The modality-neutral `Embedder` requires `embed_text`; `embed_image` accepts
existing RGBA `Frame` values and defaults to Unimplemented. Text and image
vectors from the same backend can be compared with cosine. Retrieval text uses
`task: search result | query: ` and `title: none | text: `; images have no prefix.

Per D19/D20, embeddings are used whenever the model is installed (v0.1 path;
backend lands via `embed-llama`). `Agent::new` calls `default_embedder()`;
`with_embedder` overrides that choice. This slice's small factory returns None
until slice F supplies the feature-gated runtime; it adds no native dependency.

`Agent::resolve` invokes `match_label` for click and label-based press commands,
including pre-dispatch revalidation. D21 makes the original deterministic matcher
run first: any unique valid match wins without an embedding call. Review R-E
restricts semantic fallback to queries with zero deterministic candidates of
any kind. Any case-sensitive or insensitive exact match, including disabled
or duplicate matches, and ambiguous substring matches preserve Stuck behavior.
Only then may an embedder rank enabled, nonempty labels and select the top
candidate at cosine >= `MIN_EMBED_SCORE` (0.45), with a top1 minus top2 gap >=
`MIN_EMBED_MARGIN` (0.05) when a runner-up exists. Both thresholds are provisional
until benched on both OSes. Equal scores retain tree order but fail the margin.
No embedder, embedding failure or failure of either threshold leaves the command
unmatched / Stuck. Native element identity is preserved.

Semantic targets always require confirmation: core calls the unchanged policy
classifier, upgrading Safe to Confirm while leaving Confirm and Block intact.
The confirmation detail includes the matched label and cosine score. Semantic
provenance and score are revalidated before dispatch; a change revokes approval.
Ranking never executes actions or bypasses policy or target revalidation.
VLM selection and capture remain disabled.

## Acceptance condition

Slice F integrates `llama-cpp-2` embedding mode and installed-model discovery
through `default_embedder`. Verify and pin the artifact's SHA-256 before default
download. Evaluate 256-dimensional truncation followed by L2-normalisation,
reporting the effective size through `dims()`. Cache label vectors per tree
snapshot and invalidate on snapshot, model or prefix changes. Future T1 must
map shortlisted selections back to original element indices before policy.

Benchmark shortlist recall, end-to-end selection accuracy, cold/warm latency
and memory on both macOS and Windows. Record before-and-after results from
`bench/` per rule 5 before shipping the backend. No numbers are claimed for
this factory-without-backend build.

## Consequences

- The manifest uses Apache-2.0; the obsolete Gemma Terms entry is removed.
  No weights or downloads are added in this slice.
- Malformed batches fail instead of ranking partial output. Zero-norm cosine
  similarity is zero. Native inference failures preserve deterministic fallback.
- No new tests are added under the workflow instruction. Build checks do not
  establish retrieval quality, runtime compatibility, native behavior or latency.
