# AGENTS.md

This file gives the rules for AI coding agents in the Offhand repository.
Claude Code reads it through `CLAUDE.md`. Codex and other agents read it
directly.

## What Offhand is

Offhand is an offline, open-source computer-use agent for macOS and Windows.
It acts through the operating system's accessibility tree first. It calls a
small local vision model only when the tree is not enough. Read
[`docs/architecture.md`](docs/architecture.md) before you change a crate
boundary.

## Hard rules

1. **Keep macOS and Windows equal.** A change to `offhand-platform` must ship
   with working code in `offhand-macos` and `offhand-windows`. Both must pass
   the shared contract tests. Do not merge a macOS-only feature. If a
   difference is unavoidable, record it in
   [`docs/platform-matrix.md`](docs/platform-matrix.md).
2. **Keep inference local.** Only `offhand-cloud` may open a network
   connection, and only behind the `cloud` feature. The model download in
   `xtask fetch-model` and the app updater are the two exceptions.
3. **Route every action through `offhand-policy`.** Do not call
   `Platform::act` from any other path.
4. **Prefer the tree.** Do not add a model call where an accessibility query
   can answer the question. See the `accessibility-first` skill.
5. **Measure latency changes.** A change to capture, prompts, or inference
   needs a before-and-after run from `bench/`. Put the numbers in the PR.
6. **Do not commit weights.** Add each new model to `models/manifest.toml`
   with its SHA-256 and licence.

## Commands

Run these from the repository root.

| Task | Command |
|---|---|
| Format | `cargo fmt --all` |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Test | `cargo test --workspace` |
| Licence check | `cargo deny check` |
| App dev | `npm --prefix app run tauri dev` |
| App build | `npm --prefix app run build` |
| Tasks | `cargo xtask <fetch-model\|bench\|parity-check>` |

Run format, lint, and test before you say a change is done.

## Code style

- Follow the `rust-style` guidance: the Rust API Guidelines and Microsoft's
  Pragmatic Rust Guidelines.
- Use `thiserror` for library errors and `anyhow` only in binaries.
- Do not use `unwrap` or `expect` in library code outside tests.
- Put every `unsafe` block behind a safe function. Add a `// SAFETY:` comment
  that states the invariant.
- Keep platform code inside its platform crate. Use `cfg` attributes there,
  not in `offhand-core`.

## Commits and pull requests

- Use Conventional Commits, for example `feat(macos): read AX tree`.
- Keep one logical change per commit.
- Fill in the pull request template, including the platform checklist.

## Agent personas and skills

Personas live in [`.claude/agents/`](.claude/agents/). Skills live in
[`.claude/skills/`](.claude/skills/).

| Persona | Owns |
|---|---|
| `architect` | Crate boundaries, traits, ADRs |
| `core-engineer` | `offhand-core`, `offhand-policy` |
| `macos-engineer` | `offhand-macos` |
| `windows-engineer` | `offhand-windows` |
| `inference-engineer` | `offhand-infer`, `bench/` |
| `ui-engineer` | `app/` |
| `bench-analyst` | Benchmark runs and reports |
| `reviewer` | Review of every pull request |
| `safety-reviewer` | Policy, permissions, Tauri capabilities, keys |
