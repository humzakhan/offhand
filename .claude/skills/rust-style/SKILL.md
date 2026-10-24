---
name: rust-style
description: Use when you write or review Rust in the Offhand workspace. Covers errors, unsafe, naming, public API shape, and docs.
---

# Rust style for Offhand

This is a short version of two sources. Attribution:

- Rust API Guidelines, https://rust-lang.github.io/api-guidelines/checklist.html (MIT OR Apache-2.0, Rust project developers).
- Microsoft Pragmatic Rust Guidelines, https://github.com/microsoft/rust-guidelines (MIT, Microsoft).

When these notes and the sources differ, follow `AGENTS.md`.

## Errors

- Use `thiserror` in library crates. Use `anyhow` only in binaries (`offhand-cli`, the Tauri shell, `xtask`).
- Make error types concrete and specific. Implement `std::error::Error`, `Send`, and `Sync`.
- Do not use `unwrap` or `expect` in library code outside tests. Return `Result`.
- Do not panic on bad input from the OS or the user.

```rust
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("permission missing: {0:?}")]
    PermissionMissing(Permission),
    #[error("input blocked by integrity level")]
    InputBlocked,
}
```

## Unsafe

- Put every `unsafe` block behind a safe function.
- Write a `// SAFETY:` comment on each block that states the invariant and why it holds.
- Keep the block as small as one call.

## Types

- Use newtypes for ids. Do not pass bare `u32` or `String` for a window, element, or model id.
- Add `#[must_use]` to functions and types whose result should not be ignored, such as `Result`-like guards and builders.
- Derive `Debug` on every public type. Derive `Clone`, `PartialEq`, and `Default` where they make sense.
- Mark enums and structs that may grow `#[non_exhaustive]`.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(u64);
```

## API shape

- Follow Rust naming: `snake_case` functions, `CamelCase` types, `as_`, `to_`, `into_` conversion prefixes.
- Accept `impl AsRef<str>` or slices, not `&String` or `&Vec<T>`.
- Keep fields private. Expose methods.
- Implement standard traits (`From`, `Display`, `FromStr`) instead of ad hoc methods.
- Keep `cfg` attributes inside platform crates.

## Docs

- Every public item has a doc comment with a one-line summary.
- Give a runnable example on public functions and types. Doc examples are tests.
- Document `# Errors`, `# Panics`, and `# Safety` sections where they apply.

## Checklist

- [ ] `thiserror` in libs, `anyhow` in bins
- [ ] No `unwrap` or `expect` in library code
- [ ] `// SAFETY:` on each `unsafe` block, behind a safe function
- [ ] Newtypes for ids
- [ ] `#[must_use]` and `Debug` on public types
- [ ] Doc comment and example on public items
- [ ] `cargo fmt --all` and clippy with `-D warnings` pass
