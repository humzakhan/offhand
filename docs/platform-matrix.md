# Platform matrix

The v0.1 target is equal deterministic T0 functionality on macOS and Windows.
The implementation below still needs native interactive verification; a Linux
cross-check is not a parity proof. The shared contract suite has no assertions.
`cargo xtask parity-check` is a future tool.

| Capability | macOS | Windows | Notes |
|---|---|---|---|
| Bounded external-window tree | AXUIElement | UI Automation | Up to 2,000 attempted nodes / 32 levels; cooperative 2.5 s deadline returns a partial snapshot; failing nodes/subtrees are skipped |
| Enabled label invocation | AXPress | Invoke → Toggle → SelectionItem.Select → ExpandCollapse | Unique exact → case-insensitive exact → substring; disabled exact blocks fallback |
| Secure field identity | PasswordField | PasswordField / IsPassword | Content need not be readable; typing requires confirmation |
| Text input | CGEvent Unicode | SendInput UTF-16 | Up to 2,000 Unicode characters; controls other than newline/tab rejected before dispatch; newline/tab send Return/Tab; unknown focus requires confirmation |
| Key/chord input | CGEvent | SendInput | Shared vocabulary; primary=Cmd / Ctrl; actual meta key stays distinct |
| Application launch | open -a, one name argument | ShellExecute of a resolved absolute .exe, no arguments | Bare names only; Windows rejects paths, URLs, `..`, and extensions other than `.exe`; settings apps require confirmation |
| Launch name resolution | Launch Services application names | HKCU → HKLM App Paths, then System32 / PATH excluding current and relative directories | Windows resolves only existing local-drive absolute `.exe` paths; no bare-name or file-association fallback; friendly display names may not resolve |
| Launch permission | Accessibility trust required | No AX grant | macOS checks AX trust before every action, including OpenApp |
| Window registry lifetime | Dead AX entries pruned, CF references released | Dead HWND / changed-process entries pruned | At 128 entries, evict the oldest non-target entry; retain the current target |
| Captured-window restoration | AX/native activation | HWND foreground restoration | Failure enters Stuck; never silently type elsewhere |
| Capture | Unimplemented | Unimplemented | Screen Recording is optional/future for T0 |
| Model inference/download | Unavailable | Unavailable | NoopEngine; zero progress; no model dependency or fake readiness |
| Cloud / API key | Disabled / not stored | Disabled / not stored | Feature remains default-off; no app opt-in in this release |
| Undo | Unavailable | Unavailable | Done always has undoable=false |
| AXObserver/UIA invalidation | Deferred | Deferred | Fresh snapshots at resolution/revalidation/verification boundaries |
| User-input auto-pause | Deferred | Deferred | Tray/manual Pause/Resume only; mouse movement does not pause |
| Stuck continuation | Explicit retry | Explicit retry | Cmd+Enter / Ctrl+Enter; no automatic continuation after manual input |
| Point click / scroll commands | Outside T0 grammar | Outside T0 grammar | Shared Action variants remain for future work |
| Permission prompt | macOS requests the Accessibility prompt on first denial (once per process) | No equivalent grant | Windows has no equivalent grant; the denial error tells the user to enable the host app in System Settings |
| Native grants | Accessibility required; Screen Recording optional | No equivalent grant | Windows integrity levels and foreground rules can deny input |
| Summon shortcut | Option+Space | Alt+Space | Configurable; Windows system-menu shortcut is overridden |
| Allow / Resume | Cmd+Enter | Ctrl+Enter | State-scoped bindings; plain Enter never approves |
| Stop / Skip | Escape / Tab | Escape / Tab | Escape while active; Tab only during confirmation |
| Setup | Hotkey → Permissions → Model → Try it | Hotkey → Model → Try it | Model pane honestly unavailable; no download required for tree commands |
| Tray | Menu bar | Notification area | Pause/Resume and up to three real recent results |
| Overlay coordinates | AX points converted to physical pixels | Physical UIA screen pixels | Desktop top-left origin, negative coordinates allowed; overlay converts per monitor |
| Rectangle DPI assumption | CGDisplayMode pixel width / display bounds width | UIA bounds are already desktop physical pixels | Windows assumes Per-Monitor-V2 awareness and does not rescale UIA rectangles; the CLI has no explicit DPI context; thread DPI-awareness scoping and native mixed-DPI verification are deferred |
| Content protection / click-through | Native Tauri protection | Native Tauri protection | Requires native verification; compile does not prove exclusion |
| Window material | Best-effort vibrancy | Best-effort Mica/Acrylic | Opaque graphite fallback; native appearance unverified |
| CLI | Native adapter | Native adapter | Same runner; Linux reports unsupported (exit 2) |

Existing native CI tests are retained and CI links both the app and CLI. No new
tests run in the implementation workflow. Apple cross-checks with a stub C
compiler check Rust types only; Windows cross-checks do not link or exercise an
OS session. Permission prompts, secure typing, foreground behavior, hotkeys,
content protection and mixed-DPI placement require later hands-on checks.

No capture/inference latency numbers are claimed because these execution paths
remain unavailable. Semantic task outcomes also remain unverified: Done means
the native dispatch succeeded. A failed tree refresh is reported as unverified
in Done and never leaves a retryable dispatched command.
