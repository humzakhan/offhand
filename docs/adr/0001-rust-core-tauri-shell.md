# ADR-0001: Rust core with a Tauri v2 shell

- Status: Accepted
- Date: 2026-10-24

## Context

Offhand must run on macOS and Windows. The project has a small team and must
be fast to build and to maintain. It needs a tray app, a global hotkey, a
command bar, and a click-through overlay.

## Decision

Write all logic in one Rust workspace. Use Tauri v2 with a React and
TypeScript UI for the app shell. Put platform code in `offhand-macos` and
`offhand-windows` behind the `Platform` trait.

## Consequences

- One language for the core and one UI codebase.
- No FFI layer between the core and the shell.
- The overlay depends on Tauri's `set_ignore_cursor_events` and
  `set_content_protected`. The week-1 spike tests both on both platforms.
- If the spike fails, the overlay moves to native windows that Rust creates
  through `objc2` and `windows-rs`. The rest of the decision stays.

## Alternatives

| Option | Reason not chosen |
|---|---|
| SwiftUI and WinUI shells over a Rust core | Two UI codebases and two FFI layers |
| Electron | Larger binary and memory use; no gain over Tauri |
