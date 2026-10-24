# UI design

Offhand has no main window. The screen of the app you are in is the
interface. This document is the v0.1 UI spec. The reason for the shape is in
[`adr/0004-ui-surfaces.md`](adr/0004-ui-surfaces.md). The visual reference is
[`ui/mockup.html`](ui/mockup.html): open it in any browser. Use the macOS and
Windows toggle, the tabs, or the arrow keys. A deep link such as
`mockup.html?os=win#confirm` opens one state.

## Principles

1. **One hotkey.** `⌥Space` on macOS, `Alt+Space` on Windows. Everything starts there.
2. **The screen shows the work.** The user watches the real app. Offhand adds a ring and one line of status, nothing else.
3. **The user can always stop.** `Esc` stops a run from every state. Touching the mouse or keyboard pauses it.
4. **Risky needs an explicit yes.** A confirm card never allows on plain Enter. It never times out into "allow".
5. **Local by default, cloud is visible.** A `local` badge says where inference runs. Cloud is off until the user turns it on, and violet marks every moment it is in use.

## Journeys

| ID | Journey | Path |
|---|---|---|
| J0 | First run | Install, open the app, choose the hotkey, grant permissions (macOS only), start the model download, try the first command. |
| J1 | Safe task | Hotkey, type "archive the newsletters from this week", `Enter`. Ring and pill show progress. The done pill appears, then fades. |
| J2 | Risky step | A step presses "Send". The confirm card opens next to the button. The user allows with `⌘↵` (`Ctrl+↵`), skips with `⇥`, or stops with `Esc`. |
| J3 | Trouble | The user grabs the mouse: the run pauses, and `⌘↵` (`Ctrl+↵`) resumes it. If a target cannot be found, the stuck pill asks the user to do the step, or to try cloud when it is enabled. |

J0 takes about a minute plus the download. The hotkey works while the model
downloads, because most steps resolve through the accessibility tree and need
no model call (see [`architecture.md`](architecture.md)).

## Surfaces

Four floating surfaces and two rare windows, plus the tray menu.

| # | Surface | Kind | Shown when |
|---|---|---|---|
| 1 | Command bar | Floating | The user presses the hotkey |
| 2 | Overlay (ring, status pill, done pill) | Floating, click-through | A run is active, then 3 s after it ends |
| 3 | Confirm card, block card | Floating | Policy returns Confirm or Block |
| 4 | Stuck pill, paused pill | Floating | A target is not found, or the user takes over |
| 5 | Tray / menu bar menu | Native menu | Always in the tray |
| 6 | First-run window | Window | First launch only |
| 7 | Settings window | Window | From the tray menu |

Out of scope for v0.1: a chat window, a history view, a plan editor, and
recording or replay.

### 1. Command bar

One input line, 640 px wide, in the upper third of the active display. A
hint row reads `↵ run · esc close`. A `● local` badge sits at the right of the
input. The bar has no history and no suggestions. It closes on `Esc` or when
it loses focus.

### 2. Overlay

A full-screen, transparent, click-through window that is hidden from screen
capture. It draws two things.

- **Ring.** A 2 px ring with a soft glow around the element Offhand is about
  to act on. White for a normal step. Amber for confirm, red for block,
  violet while the cloud engine is acting.
- **Pill.** One line, centred at the bottom of the display, 20 px above the
  edge (above the taskbar on Windows). Running: `◐ Archiving 3 of 7 · esc stop`.
  Done: `✓ Archived 7 newsletters · ⌘Z undo`.

Undo shows only when every step of the run was reversible. The done pill fades
after about 3 seconds. A thin line under the pill shows the countdown.

### 3. Confirm and block cards

The confirm card sits next to the risky target, never in the centre of the
screen. Example: `⚠ Press "Send" in Mail? to finance@acme.com`, with
`⌘↵ allow · ⇥ skip · esc stop`. Plain `Enter` never allows.

The block card is red, has one `OK` button (key hint `esc`), and has no allow
action. `OK` dismisses the card and stops the run.

### 4. Stuck and paused pills

- Stuck: `? Can't find "Export". Click it for me, or esc to stop.` plus
  `☁ try cloud` in violet. The cloud button appears only when cloud is enabled.
- Paused: `⏸ Paused — you took over · ⌘↵ resume · esc stop`. It appears when
  the user touches the mouse or keyboard during a run.

### 5. Tray / menu bar menu

A native menu with these rows, in this order.

| Row | Notes |
|---|---|
| Run command… | Shows the hotkey |
| Recent (3) | `✓` done, `✕` failed or stopped, with age |
| ● Local · Qwen3.5-4B | Status line, not clickable. Shows `☁ Cloud` in violet while cloud runs |
| Settings… | `⌘,` on macOS |
| Quit Offhand | `⌘Q` on macOS |

### 6. First-run window

Four steps on macOS, three on Windows.

| Step | Content | Platform |
|---|---|---|
| Hotkey | Shows the default. The user can change it | Both |
| Permissions | Accessibility and Screen Recording, with live status | macOS only |
| Model | Progress bar for `Qwen3.5-4B · 2.9 GB · runs fully offline`, speed, time left | Both |
| Try it | The hotkey and a sample prompt | Both |

The user can continue to "Try it" while the download runs.

### 7. Settings window

One screen, five cards: Model (picker, size, status), Hotkey, Safety (summary
of confirm rules, read-only in v0.1), Cloud (off by default, opt-in toggle,
bring-your-own key), Permissions (status). The Windows build shows "Nothing to
grant" in the Permissions card.

## State to surface

The core reports one state at a time. The UI maps each to a surface.

| Core state | Surface |
|---|---|
| Idle | None (tray only) |
| Listening | Command bar |
| Running { step, total, target } | Ring and running pill |
| AwaitConfirm { label, app, detail } | Confirm card and amber ring |
| Blocked { label } | Block card and red ring |
| Stuck { query, cloud_available } | Stuck pill |
| Paused | Paused pill |
| Done { summary, undoable } | Done pill |

## Interaction rules

| Rule | Value |
|---|---|
| Stop | `Esc` in every state except Idle. It stops the run and closes the surface |
| Allow | `⌘↵` on macOS, `Ctrl+↵` on Windows |
| Skip | `⇥` skips the step and the run continues |
| Resume | `⌘↵` on macOS, `Ctrl+↵` on Windows, on the paused pill |
| Plain Enter | Never acts globally. `Enter` works only inside Offhand's own focused windows (command bar, first run, settings) |
| Auto-pause | Any mouse or keyboard input that Offhand did not generate pauses the run before the next action |
| Done fade | About 3 s. The countdown line is visible |
| Confirm timeout | None. The card stays until the user answers. Closing it counts as stop |
| Stuck timeout | None. It stays until the user acts or stops |
| Focus | The overlay never takes focus. Cards and pills take keys through shortcuts that are registered only while they are visible |
| Capture | Command bar, overlay, cards, and pills are hidden from screen capture |

## Platform parity

The layout is identical on both platforms. These differences are unavoidable
and belong in [`platform-matrix.md`](platform-matrix.md).

| Item | macOS | Windows |
|---|---|---|
| Hotkey | `⌥Space` | `Alt+Space` |
| Modifier in hints | `⌘`, `⌥` | `Ctrl`, `Alt` |
| Permissions step | Accessibility, Screen Recording | None |
| Tray | Menu bar, top right | Notification area, bottom right |
| Pill offset | 20 px above the screen edge | 20 px above the taskbar |
| Window material | Vibrancy | Mica or Acrylic |

On Windows, `Alt+Space` normally opens the window's system menu. Offhand's
global hotkey takes it over, so the user loses that menu while Offhand runs.
First run lets the user pick another hotkey, and Settings can change it later.

## Design tokens

Warm graphite neutrals. Colour marks state only.

### Colour

| Token | Value | Use | Contrast on surface |
|---|---|---|---|
| `bg` | `#0C0C0D` | Page and window background | n/a |
| `surface` | `#161618` | Cards | n/a |
| `raised` | `#1D1D20` | Controls on cards, keycaps | n/a |
| `hairline` | `#2A2A2E` | 1 px borders | n/a |
| `text` | `#EDEDEF` | Primary text | 15.5 : 1 |
| `muted` | `#9A9AA3` | Labels, hints, secondary text | 6.5 : 1 |
| `faint` | `#6C6C75` | Separators, disabled. Not for essential text | 3.5 : 1 |
| `amber` | `#F2B04C` | Confirm | 9.5 : 1 |
| `red` | `#FF7B72` | Block, failed | 7.2 : 1 |
| `green` | `#7CC79A` | Done, granted | 9.0 : 1 |
| `violet` | `#B3A6F8` | Cloud in use | 8.4 : 1 |

The `local` dot is `text` at 75 % opacity. It is not green, because green
means done. Floating surfaces use `rgba(24,24,27,.74)`, a 28 px backdrop blur,
a 1 px edge at 9 % white, and a 1 px inner highlight on the top edge.

### Type

Geist Sans for UI. Geist Mono for key hints, numbers, and sizes.

| Step | Size / weight | Use |
|---|---|---|
| Display | 56 / 600 mono | Download percent |
| Hero | 32 / 600 mono | Card figures |
| Title | 22 / 600 | Window step titles |
| Input | 20 / 400 | Command bar text |
| Body | 14 / 400 | Paragraphs |
| UI | 13 / 500 | Buttons, rows, pills |
| Caption | 12 / 400 | Hints, metadata |
| Key | 11 / 500 mono | Keycaps |

### Shape and space

| Token | Value |
|---|---|
| Radius: command bar | 16 px |
| Radius: cards, windows | 14 px |
| Radius: buttons | 9 px |
| Radius: keycap | 6 px |
| Radius: pills, badges | full |
| Spacing scale | 4, 8, 12, 16, 20, 24, 32 px |
| Card padding | 20 px (confirm card 16 px) |
| Ring | 2 px, 6 px outside the element, 2 s slow pulse |
| Shadow | `0 20px 56px rgba(0,0,0,.5)` on floating surfaces |

Motion is limited to the ring pulse, the progress spinner, the caret, and the
done countdown. All of it stops under `prefers-reduced-motion`.

## Font licence

Geist Sans and Geist Mono are by Vercel, with basement.studio. They use the
SIL Open Font License 1.1, which allows bundling and embedding. The
mockup embeds the variable woff2 files from the `geist` npm package, version
1.8.2. The licence text is in [`ui/GEIST-OFL.txt`](ui/GEIST-OFL.txt). The app
must bundle the files locally. It must not load fonts over the network.

## Open questions

- `offhand_policy::classify` returns `Safe` or `Confirm` only. Nothing produces
  `Block` yet, so the block card has no trigger. The rule set needs a decision.
- Undo needs a reversibility flag on each step. `Action` has none today.
