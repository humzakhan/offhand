# Local models

## Enable the embedder

Download the optional Apache-2.0 Q8_0 text tower:

```sh
cargo xtask fetch-model embeddinggemma-2
```

The task reads `models/manifest.toml`, prints the model's licence, streams the
GGUF from its pinned repository revision, and verifies and prints its SHA-256.
The EmbeddingGemma 2 checksum comes from the file's LFS `oid` in the
[Hugging Face tree API](https://huggingface.co/api/models/unsloth/embeddinggemma-2-GGUF/tree/031f0d4b35536f69ab3509d4893c923264fcf253).
An existing file is hashed and verified before reuse. A manifest checksum of
`TBD` is a hard error unless `--allow-unverified` is explicitly passed; that
option warns for both downloads and existing files. A full repository commit
SHA remains required even with that option. Set `OFFHAND_MODELS_DIR` to choose the
directory; otherwise both the downloader and backend use the platform's local
data directory plus `offhand/models` (`~/Library/Application Support` on macOS,
`%LOCALAPPDATA%` on Windows, `$XDG_DATA_HOME` or `~/.local/share` on Linux).

Enable the backend when building the app:

```sh
cargo build -p offhand-app --features offhand-infer/embed-llama
```

This optional feature requires CMake, a C++ compiler, and libclang for the native
bindings. It is off by default. Inference stays local and text-only, with mean
pooling, a 2048-token context, and 256-dimensional Matryoshka vectors normalised
after truncation. The caller adds query/document prefixes once; image and audio
embedding remain unsupported. CPU inference is the portable default.

**Current runtime blocker:** the newest published `llama-cpp-2`/`llama-cpp-sys-2`
release is pinned to `=0.1.160`. Its llama.cpp submodule is
`d81235049384534c167caea52b85a694f6103d14`, which is 23 commits before
`4fbc76d` (PR #30054, `embeddinggemma2` support). The binding repository's main
branch (`064f9e1ffc852b58de9b6fcec4c0ac1357c62890`, checked 2026-10-24) has
the same submodule, and no newer tag exists. Loading EmbeddingGemma 2 therefore
fails gracefully: the factory logs the error and returns `None`, retaining tree
matching. A missing model also returns `None`. Update the binding to one that
includes #30054 before expecting semantic matching. Real model inference,
retrieval quality, and latency have not been verified or benchmarked.
