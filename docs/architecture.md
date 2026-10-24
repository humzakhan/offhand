# Architecture

Offhand v0.1 implements an accessibility-first runner. Each run
contains one explicit `open`, `click`, `type` or `press` command. The broader
local-model ladder is a future design, not an enabled fallback. The visual
reference is [ui.md](ui.md); deliberate v0.1 limits are recorded below and in
[platform-matrix.md](platform-matrix.md).

## Boundaries

| Component | Owns | Calls |
|---|---|---|
| `offhand-platform` | Platform trait, native types and grant vocabulary | No OS implementation |
| `offhand-macos` | AX tree/press, CGEvent input, captured-window activation | macOS APIs |
| `offhand-windows` | UIA tree/invoke, SendInput, captured-window activation | Windows APIs |
| `offhand-policy` | Exhaustive Safe / Confirm / Block classification | Shared types only |
| `offhand-infer` | Engine and Embedder traits, default embedder factory and no-op backends | Local label ranking when a backend is installed |
| `offhand-cloud` | Optional future remote inference | Feature `cloud`, default-off |
| `offhand-core` | Parsing, pinned target, staged runner, approval identity, UI state | Platform, policy, Engine interface |
| `offhand-cli` | Native adapter selection, terminal approval and Ctrl+C | Same Agent + NoopEngine |
| `app/src-tauri` | Actor, shortcuts, tray, permissions, persistence and IPC | Same Agent + NoopEngine |
| React surfaces | Local presentation and explicit commands | Restricted per-window IPC |

Only core dispatches `Platform::act`, and every dispatch is gated by policy.
There are no OS `cfg` branches in core and no Send/Sync requirement on Platform.
Windows COM-bearing state stays on its owning thread. The desktop actor
constructs the native adapter inside that thread, drains controls before ticks,
and forwards subscribed states. The shell must not hold Agent in a cross-thread
Mutex or dispatch actions directly.

The app has exactly five labels: `command`, `overlay`, `confirm`, `first-run`
and `settings`; there is no main window. Capabilities and handler-side caller
checks limit each label's commands. The overlay only listens to state events.
Mutating invokes acknowledge actor acceptance, not semantic task completion.

## Staged T0 loop

1. `listen` records the external foreground window before command-bar activation.
   Direct CLI `submit` captures it automatically. `open` works without a window.
2. `submit` replaces all prior work and approvals. Resolve the command against a
   fresh, bounded snapshot of that captured window. Match labels uniquely first.
   If unmatched or ambiguous, a local embedder may select the top enabled target
   at cosine >= 0.45 (provisional until benched). Errors or lower scores remain
   unmatched. Never guess coordinates.
3. Publish Running with the highlight, AwaitConfirm with a new increasing ID,
   Blocked, or Stuck. No action is dispatched on the resolution tick.
4. A later tick restores the captured window through a policy-approved
   FocusWindow action if necessary. This consumes that tick's one native action.
   Allow up to five subsequent ticks for foreground activation to settle;
   failure enters Stuck. Re-resolve the action/target before dispatch; changes
   revoke any approval and re-enter policy. Three consecutive revalidation
   mismatches enter Stuck rather than looping indefinitely.
5. Dispatch at most one action per tick, then clear the command before refreshing
   the tree. Done reports API dispatch, never semantic verification. A failed
   refresh produces Done with an explicitly unverified result; neither Resume nor
   another tick can repeat a successful dispatch.

A command `click <label>` means accessibility Press/Invoke, not coordinate input.
`press` first recognizes the shared key vocabulary, then tries an enabled label.
Unsupported chords and reserved Escape enter Stuck. Without an embedder,
ambiguous/missing labels enter Stuck; semantic ranking may resolve those labels
when an embedder is installed.
The shell rejects injecting its own configured summon chord. Typing suffixes are
preserved verbatim; application launch names are trimmed and never placed in state messages or diagnostic Agent output.

Paused, AwaitConfirm, Blocked and Stuck ticks perform no action. Stop clears all
pending work and emits Idle. Resume from Paused or Stuck explicitly starts fresh
resolution and revokes approval. A confirmation response must match the currently
pending globally increasing ID. Allow queues dispatch for a later tick; Skip
completes this single-step run; Stop clears it. Block has no allow path. Approval
never carries to a changed target, a replacement command or another step.

Policy confirms risky labels, password-role or password-label typing, unknown
focused targets, multiline text, non-navigation keys/chords and system-settings
launches. Permanent empty-trash/recycle-bin labels are blocked. App names reject
URLs, paths, control characters and option prefixes. Native launchers receive
one name argument (macOS `open -a`) or ShellExecute with no extra arguments,
never a shell command string.

## Native snapshot and coordinate contract

`tree(None)` reads the external application's focused window; `tree(Some(id))`
is pinned even while Offhand is visible. Adapters exclude Offhand's own process,
retain native references for pending IDs, cap traversal at 2,000 nodes and 32
levels, and use native call timeouts where available. `focused_element` preserves
PasswordField even when secure content is unreadable.

Rect and public Bounds use desktop physical pixels, top-left origin, including
negative coordinates. macOS AX points are converted with the owning display's
scale; Windows UIA bounds use physical screen coordinates. The overlay subtracts
its monitor origin and divides by scale to draw CSS pixels. Native point-input
adapters must invert the same conversion.

macOS Accessibility grants are required for actions; Screen Recording is an
optional future capture grant. Windows returns None for both grants and cannot
bypass integrity levels, foreground restrictions or rejected SendInput calls.

## Current limits versus future design

Local embeddings are defined in [ADR-0005](adr/0005-embeddings.md). Agent loads
the default embedder when available and uses semantic label ranking only when
deterministic matching cannot resolve a unique target. The provisional minimum
cosine is 0.45; policy and confirmation still apply. With no backend, on an
embedding error or below that score, the command stays unmatched. Future T1 can
reuse the shortlist before `Engine::choose`.

Capture returns Unimplemented on both OSes. NoopEngine returns Unimplemented for
both Engine methods. This slice adds no llama.cpp dependency; the default
embedder factory returns None until a backend is integrated. Model
onboarding/settings report unavailable with zero progress; cloud remains disabled
and the app stores no API key. No undo is offered (`undoable=false`).

AXObserver/UIA invalidation subscriptions and global user-input auto-pause hooks
are deferred on both platforms. Fresh bounded snapshots replace event-driven
cache invalidation. Pause/Resume is manual, and Stuck retries only after an
explicit user action. Point clicks and scrolling are outside this release's
command grammar even though the shared trait retains those future actions.

The future ladder is T0 tree → T1 local element selection → T2 local visual
grounding → optional T3 cloud. The reasons are in [thesis.md](thesis.md) and
[adr/](adr/). Only offhand-cloud may make runtime network connections, behind
its feature; explicit model-fetch tooling and the app updater are exceptions.
The seam-only build performs neither inference nor capture and claims no latency
benchmark numbers. Enabling an embedding backend requires before-and-after
benchmarks on both OSes.

## Evidence limits

The shared contract suite is a placeholder with no assertions. Compilation and
existing-test target compilation do not prove native parity. Native CI retains
existing tests and explicitly builds the desktop app and CLI on macOS and Windows.
Linux cross-checks cannot prove native linking or desktop behavior; the Apple
check using a C compiler stub checks Rust cfgs only.

Hands-on native checks remain necessary for AX/UIA behavior, foreground
restoration, integrity denial, permissions, secure fields, hotkeys, focusless and
capture-protected windows, mixed-DPI placement, materials and semantic outcomes.
There are no new tests in this implementation workflow.
