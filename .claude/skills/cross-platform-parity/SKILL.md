---
name: cross-platform-parity
description: Use when you change the Platform trait in offhand-platform, add a platform feature, or touch offhand-macos or offhand-windows. Keeps macOS and Windows equal.
---

# Cross-platform parity

Offhand ships on macOS and Windows with the same features. A change that works on one side only is not done.

## Order of work

Follow this order. Do not skip a step.

1. **Change the trait** in `crates/offhand-platform/`. Write the ADR in `docs/adr/` first if the change moves a boundary.
2. **Change both adapters:** `offhand-macos` and `offhand-windows`. Land them in the same pull request.
3. **Change the contract tests** in `offhand-platform`. Both adapters run the same tests.
4. **Update `docs/platform-matrix.md`** with the status of each side.

## Name the Windows call first

Before you design a method, name the call on each OS.

| Need | macOS | Windows |
|---|---|---|
| Read the tree | `AXUIElement` | UI Automation |
| Capture a window | ScreenCaptureKit | Windows.Graphics.Capture |
| Send input | `CGEvent` | `SendInput` |
| Window-change event | `AXObserver` | UIA events |

If one side has no call, the method is not ready. Redesign it or record the gap in the matrix.

## Where `cfg` is allowed

Use `#[cfg(target_os = "...")]` only in platform crates. `offhand-core`, `offhand-policy`, and `offhand-infer` must compile on both without it. Core talks to `dyn Platform`.

```rust
// Good: in offhand-macos
#[cfg(target_os = "macos")]
mod ax;

// Bad: in offhand-core
#[cfg(target_os = "macos")]
fn pick_backend() { /* ... */ }
```

## Contract tests

Write each test once against the trait. Run it for each adapter.

```rust
pub fn contract_click_reports_blocked_input<P: Platform>(p: &P) {
    // The same assertion runs on macOS and on Windows.
}
```

A silent failure is a parity bug. Windows `SendInput` returns 0 with no error when UIPI blocks it. macOS drops events without a TCC grant. Both must return the same typed error.

## CI

Both legs are required: macOS and Windows. Do not merge on one green leg. Do not mark a leg `continue-on-error`.

## Checklist

- [ ] ADR written if a boundary moved
- [ ] Trait changed in `offhand-platform`
- [ ] `offhand-macos` implements it
- [ ] `offhand-windows` implements it
- [ ] Contract tests cover it and pass on both
- [ ] `docs/platform-matrix.md` updated
- [ ] No `cfg(target_os)` outside platform crates
- [ ] Both CI legs green
- [ ] `cargo fmt --all`, clippy with `-D warnings`, and `cargo test --workspace` pass
