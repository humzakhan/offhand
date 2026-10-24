# Architecture

This document describes the components of Offhand, their boundaries, and the
step loop. The reasons for each choice are in [`thesis.md`](thesis.md) and in
[`adr/`](adr/). The UI surfaces are specified in [`ui.md`](ui.md).

## Components

```
┌──────────────────────────────────────────────────────────────────────────┐
│                        OFFHAND  (Tauri v2 app)                           │
│  ┌──────────────┐  ┌──────────────────┐  ┌──────────────┐  ┌──────────┐  │
│  │ Command bar  │  │ Overlay (click-  │  │ Settings /   │  │ Tray /   │  │
│  │ hotkey       │  │ through, hidden  │  │ model picker │  │ menu bar │  │
│  │              │  │ from capture)    │  │              │  │          │  │
│  └──────┬───────┘  └────────▲─────────┘  └──────┬───────┘  └────┬─────┘  │
│         └───────── React + TS (Vite) ───────────┴───────────────┘        │
│                          │  Tauri commands + events                      │
├──────────────────────────┼───────────────────────────────────────────────┤
│                          ▼          RUST CORE (cargo workspace)          │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │ offhand-core   step loop · tier router · task state · element cache│  │
│  └───┬───────────────────┬──────────────────────┬──────────────────┬──┘  │
│      ▼                   ▼                      ▼                  ▼     │
│ ┌──────────┐   ┌──────────────────┐   ┌──────────────────┐ ┌──────────┐  │
│ │ policy   │   │ platform (trait) │   │ infer (trait)    │ │ cloud    │  │
│ │ Safe     │   │ tree()           │   │ choose(options)  │ │ BYO key  │  │
│ │ Confirm  │   │ capture()        │   │ ground() → (x,y) │ │ feature  │  │
│ │ Block    │   │ act()            │   │ llama.cpp mtmd   │ │ "cloud"  │  │
│ └──────────┘   └────┬─────────┬───┘   └──────────────────┘ └──────────┘  │
│                     ▼         ▼                                          │
│           ┌──────────────┐ ┌───────────────┐                             │
│           │ offhand-macos│ │offhand-windows│                             │
│           │ AXUIElement  │ │ UI Automation │                             │
│           │ CGEvent      │ │ SendInput     │                             │
│           │ ScreenCapKit │ │ WGC capture   │                             │
│           └──────────────┘ └───────────────┘                             │
└──────────────────────────────────────────────────────────────────────────┘
```

## Crates

| Crate | Responsibility | Depends on |
|---|---|---|
| `offhand-platform` | `Platform` trait, shared types, contract tests | none |
| `offhand-macos` | macOS implementation of `Platform` | platform |
| `offhand-windows` | Windows implementation of `Platform` | platform |
| `offhand-policy` | Risk classification of each action | platform |
| `offhand-infer` | `Engine` trait and the llama.cpp backend | platform |
| `offhand-cloud` | Optional remote engine (feature `cloud`) | infer |
| `offhand-core` | Step loop and tier router | platform, policy, infer |
| `offhand-cli` | `offhand` command for scripts, bench, and CI | core |
| `app/src-tauri` | Desktop app shell | core |

`offhand-core` does not contain platform `cfg` attributes. The app and the CLI
select the platform crate at compile time.

## Step loop

```
  task
    │
    ▼
  T0  read tree (cached until a window-change event)
    │   target resolved? ── yes ──▶ policy ──▶ act ──▶ verify
    │ no
    ▼
  T1  crop to window ─▶ candidates from tree ─▶ Engine::choose
    │   prob ≥ threshold? ── yes ──▶ policy ──▶ act ──▶ verify
    │ no
    ▼
  T2  Engine::ground ─▶ (x, y) ─▶ policy ──▶ act ──▶ verify
    │ two failures, or a multi-step plan is needed
    ▼
  T3  cloud engine (only if the feature is on and the user gave a key)
```

Verification reads the tree first. It captures a new frame only if the tree
shows no change.

## Platform contract

Each platform crate must pass `offhand_platform::contract::run_contract_tests`.
The contract tests use the same fixtures on both operating systems. A
difference in behaviour goes into [`platform-matrix.md`](platform-matrix.md).

## Data boundaries

| Data | Stays on device | Leaves device |
|---|---|---|
| Screen frames | Always, unless T3 is on | Only to the T3 provider the user chose |
| Accessibility tree | Always, unless T3 is on | Only to the T3 provider the user chose |
| Model weights | Downloaded once from Hugging Face | No |
| Telemetry | None | No |
