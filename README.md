<div align="center">

# Offhand

**An offline accessibility agent for macOS and Windows.**

Run one explicit command against the app you are using.
Your screen and text stay on your device.

[![ci](https://github.com/humzakhan/offhand/actions/workflows/ci.yml/badge.svg)](https://github.com/humzakhan/offhand/actions/workflows/ci.yml)
![platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Windows-informational)
![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)
![status](https://img.shields.io/badge/status-pre--alpha-orange)

</div>

## v0.1: tree commands

This pre-alpha implements T0: one deterministic command per run, using the
operating system's accessibility tree. It is not a natural-language planner.
There is no model inference, model download, cloud fallback or undo in this
release. The model pane reports unavailable and cloud controls stay disabled.

Supported commands:

```text
open Calculator
click Save
type Text to insert
press primary+s
press Export
```

Leading command whitespace is ignored. Verbs are case-insensitive; the text
after the verb's first separator is preserved verbatim for typing; application
names are trimmed before launch. `click` invokes an
enabled accessibility element; it does not guess coordinates. Label matching
requires a unique exact match, then a unique case-insensitive exact match,
then a unique case-insensitive substring. Disabled exact matches stop resolution.
`press` prefers a supported key/chord and otherwise resolves an enabled label.
Ambiguous, missing and unsupported targets stop in Stuck for explicit retry.

Keys: Enter, Tab, Space, Backspace, Delete, Left, Right, Up, Down, Home, End,
PageUp, PageDown, F1–F12 and single ASCII letters/digits. Modifiers are `ctrl`,
`alt`, `shift`, `meta` and `primary`, joined with `+`. `primary` is Cmd on macOS
and Ctrl on Windows; `meta` means the actual OS meta key. Escape/Esc is reserved
for stopping Offhand. The desktop shell also rejects its summon shortcut as
an injected key.

## Desktop and CLI

The desktop shell lives in the tray/menu bar with five surfaces: command bar,
click-through overlay, confirmation card, first-run setup and Settings. Summon
with Option+Space on macOS or Alt+Space on Windows; choose another shortcut in
Settings if it conflicts with an existing binding. Windows normally uses
Alt+Space for its window menu.

Capture the target before the command bar activates. Offhand highlights a
resolved element before dispatch and restores the captured external window
before input. It stops if restoration or revalidation fails. Every native
action goes through the policy gate.

- Sends, deletes, purchases and other risky labels require confirmation.
- Password fields, unknown focused typing targets, multiline text, activating
  keys/chords and system-settings launches require confirmation too.
- Empty-trash/recycle-bin actions are blocked with no Allow path. App launch
  names cannot be URLs, paths, options or contain control characters.
- Desktop Allow uses Cmd+Enter / Ctrl+Enter or its explicit button; plain Enter
  never approves. Escape stops. Pause/Resume is available from the tray.

The CLI runs the same Agent and NoopEngine on the native owning thread:

```sh
cargo run -p offhand-cli -- run "open Calculator"  # macOS
cargo run -p offhand-cli -- run "open notepad"    # Windows
cargo run -p offhand-cli -- run "click Save"
```

CLI approval requires typing `allow` at an interactive terminal. Blank input,
other text and non-TTY input deny; there is no `--yes`. Ctrl+C requests stop
before the next action. Exit codes: 0 completed/skipped, 1 stuck/platform failure,
2 blocked/unsupported OS, 130 stopped. Linux compiles but reports unsupported.

A Done state means the OS accepted the dispatch. If a tree refresh fails, Done
explicitly reports the result as unverified; Resume never repeats that dispatch.
Even a successful refresh does **not** prove a document was saved, a message
delivered, or another semantic outcome. Verify results in the target app; there is no automatic undo.

## Grants and deliberate limits

macOS needs Accessibility permission for tree/input actions. Screen Recording is
optional/future for T0. Windows UI Automation needs no equivalent grant, but
higher-integrity applications and foreground restrictions may reject input.
Native grants and behavior must be checked on actual macOS and Windows sessions.

Both platforms deliberately leave capture unimplemented. AXObserver/UIA change
subscriptions and global user-input auto-pause hooks are deferred; the runner
uses bounded fresh snapshots at resolution and verification boundaries. Moving
the mouse does not automatically pause this release. Use manual Pause/Resume.
Stuck requires an explicit retry (Cmd/Ctrl+Enter in the shell), not an assumed
automatic continuation after a manual click. Point clicks, scrolling and visual
fallbacks are outside the T0 command vocabulary.

Bounds are desktop physical pixels with top-left origin, including negative
coordinates on secondary displays. Adapters normalize native coordinates; the
overlay converts using its monitor origin and scale. Native content protection,
focusless cards, global shortcuts, mixed-DPI positioning and window materials
need hands-on verification; successful compilation does not prove them.

The shared contract suite currently asserts nothing. Cross-checks check Rust
code under native cfgs; they do not link or exercise desktop sessions. Native CI
keeps existing tests and builds both the app and CLI. No tests were added for
this slice. No capture/inference latency measurements are claimed: both paths
remain unavailable and T0 calls neither. Future tiers and latency targets live
in the [architecture](docs/architecture.md), [thesis](docs/thesis.md) and
[roadmap](docs/roadmap.md).

## Build from source

Use Rust 1.94 and Node 22.12 or newer. macOS needs Xcode Command Line Tools;
Windows needs MSVC Build Tools and WebView2. Linux is only a compile target for
the unsupported shell and needs the Tauri WebKit/system prerequisites.

```sh
git clone https://github.com/humzakhan/offhand
cd offhand
pnpm --dir app install --frozen-lockfile
pnpm --dir app run build
cargo build -p offhand-app -p offhand-cli
pnpm --dir app run tauri dev
```

## Repository layout

```text
crates/
  offhand-core       staged single-command runner and public UI state
  offhand-policy     exhaustive action classification and confirmation rules
  offhand-platform   shared native contract and placeholder contract suite
  offhand-macos      AXUIElement, CGEvent and application activation
  offhand-windows    UI Automation, SendInput and application activation
  offhand-infer      Engine trait and deliberately unavailable NoopEngine
  offhand-cloud      optional future cloud tier, default-off
  offhand-cli        offhand run command
app/                 Tauri v2 / React desktop shell
bench/               future latency and accuracy harness
docs/                design, architecture, ADRs and roadmap
```

Weights are never bundled or committed. Future models require a SHA-256 and
licence in [`models/manifest.toml`](models/manifest.toml); the v0.1 app does not
download them. The app does not accept or store a cloud API key.

## Contributing and licence

Read [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md). Shared-host
execution restrictions still apply to agent workflows; the commands above do
not grant permission to run workloads on a shared host.

Offhand is dual-licensed under [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE). Model weights carry their own licences.
