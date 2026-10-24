# Contributing to Offhand

Thanks for looking at Offhand. Keep changes focused and explain the why in the PR.

## Prerequisites

- Rust 1.94, installed via [rustup](https://rustup.rs)
- Node 22 or later
- pnpm 10+ (`npm i -g pnpm` or see pnpm.io/installation)
- macOS: Xcode Command Line Tools
- Windows: MSVC Build Tools and the WebView2 runtime

## Build

```sh
pnpm --dir app install
pnpm --dir app run build
cargo check --workspace
pnpm --dir app run tauri dev
```

## Before you open a PR

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
```

## Cross-platform rule

Every change to `offhand-platform` must land with working macOS and Windows implementations. CI runs on both and both are required.

## Commit style

Use [Conventional Commits](https://www.conventionalcommits.org), for example `feat(policy): add deny rule for clipboard writes`.

## Licensing

Contributions are dual-licensed under MIT OR Apache-2.0, as described in [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
