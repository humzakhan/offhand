# ADR-0004: UI surfaces

- Status: Proposed
- Date: 2026-10-24

## Context

Offhand acts inside other apps. The user's attention belongs on the app that
is changing, not on Offhand. A main window would pull attention away from the
work, and would sit in front of the thing being operated.

The shell is Tauri v2 (ADR-0001). The overlay must be click-through and
hidden from capture. The UI needs to look the same on macOS and Windows.

## Decision

Offhand has no main window. The screen of the app the user is in is the UI.

Four floating surfaces:

1. Command bar, summoned by `⌥Space` (macOS) or `Alt+Space` (Windows).
2. Overlay: a click-through ring around the target and a one-line status pill.
3. Confirm card, anchored next to the risky target. A red block variant has no
   allow action.
4. Stuck and paused pills.

Plus the tray menu, and two rare windows: first run and settings.

`Esc` stops a run from every state. Any real user input pauses it. A risky
action needs an explicit yes, and plain `Enter` never gives it. The full spec
is in [`../ui.md`](../ui.md). The visual reference is
[`../ui/mockup.html`](../ui/mockup.html).

## Consequences

- **Several Tauri windows.** The app creates a command window, a full-screen
  overlay window, a small confirm-card window, and on demand the first-run and
  settings windows. The tray owns the app lifetime. Closing a window never
  quits.
- **A capability file per window.** The overlay only listens to events and
  invokes nothing. The command bar may submit a command. The confirm card may
  only answer the pending confirmation. Settings and first run get their own
  commands. No window gets file, shell, or network access (see the
  `tauri-v2-security` skill).
- **Content protection.** The command bar, overlay, and cards call
  `set_content_protected`, so the vision model never sees Offhand's own UI.
  This extends the week-1 spike in ADR-0001 to more than one window per
  platform.
- **Click-through and input are separate.** The overlay ignores the cursor, so
  it cannot hold buttons. The confirm card is its own window. It must not take
  focus from the target app, so its keys (`⌘↵`, `⇥`, `Esc`) come from global
  shortcuts registered only while the card is visible. The paused pill works
  the same way, with `⌘↵` to resume. Plain `Enter` is never a global
  shortcut, so it cannot collide with the user's typing. The spike must show
  this works without activating the window on both platforms.
- **Translucent surfaces on both platforms.** The floating windows use
  transparency with window effects: vibrancy on macOS, Mica or Acrylic on
  Windows. Verify in the spike whether Tauri's built-in window effects are
  enough or an extra plugin is needed. Each surface needs an opaque fallback
  when the effect is unavailable. Transparent windows on macOS may need
  Tauri's `macOSPrivateApi` setting, which is acceptable for a signed `.dmg`
  but rules out the Mac App Store.
- **Bundled fonts.** Geist Sans and Geist Mono ship inside the app under the
  SIL OFL. The CSP stays closed to remote fonts.
- **The core needs a UI state stream.** Idle, Listening, Running, AwaitConfirm,
  Blocked, Stuck, Paused, and Done. Each maps to one surface. Undo needs a
  reversibility flag per step, which `Action` does not have yet.
- **`Alt+Space` overrides the Windows system menu.** The hotkey replaces the
  window menu shortcut while Offhand runs. First run lets the user change it.
- **Parity cost is small.** Both platforms use the same React code. Only the
  hotkey, the modifier label, the permissions step, and the tray position
  differ. Record them in `platform-matrix.md`.

## Alternatives

| Option | Reason not chosen |
|---|---|
| Chat window with a message history | Moves attention off the screen being operated. Adds history, scroll, and a conversation model that v0.1 does not need |
| Dashboard main window | A window to open and manage. It shows data users do not need, and competes with the app being operated |
| Tray popover with an input | Takes focus from the target app and covers the menu bar area. No place to show a ring on the target |
| Native SwiftUI and WinUI surfaces | Two UI codebases. Same objection as in ADR-0001 |
