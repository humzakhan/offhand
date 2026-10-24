<div align="center">

# Offhand

**A fast, offline computer-use agent for macOS and Windows.**

Tell Offhand what to do. It clicks, types, and scrolls for you.
Your screen never leaves your machine.

[![ci](https://github.com/humzakhan/offhand/actions/workflows/ci.yml/badge.svg)](https://github.com/humzakhan/offhand/actions/workflows/ci.yml)
![platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Windows-informational)
![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)
![status](https://img.shields.io/badge/status-pre--alpha-orange)

</div>

---

> **Status: pre-alpha.** The repository holds the plan, the architecture, and a
> compiling scaffold. Nothing controls your computer yet. See the
> [roadmap](docs/roadmap.md).

## Why Offhand

Most computer-use agents send a screenshot to a large cloud model for every
click. That is slow, it costs money per step, and it sends your screen to a
third party. Local agents that copy this loop are slower still: a 7B model on a
24 GB Mac takes 17 to 35 seconds per step.

Offhand uses a different loop. It reads the operating system's accessibility
tree first and acts with no model at all when it can. It calls a small local
vision model only when the tree is not enough. Even then, the model picks one
option from a short list. It does not write paragraphs.

## How it works

```
  "export this invoice as PDF"
               │
               ▼
  T0  accessibility tree + shortcuts ──hit──▶ act            target < 50 ms
               │ miss
               ▼
  T1  crop window → model picks option #N ──▶ act            target < 400 ms
               │ target not in tree
               ▼
  T2  model grounds → (x, y) ───────────────▶ click          target < 1.5 s
               │ stuck, or long-horizon plan
               ▼
  T3  cloud model (opt-in, your own key, off by default)

  Every tier passes through the action policy. Sends, deletes,
  purchases, and password fields always ask you first.
```

The latency numbers are design targets. The benchmark harness in
[`bench/`](bench/) measures them before any tier ships.

## Principles

- **Offline first.** All perception and inference run on your device. The cloud
  tier is a compile-time feature and is off by default.
- **Accessibility before pixels.** The tree is faster and more exact than
  vision. Vision is the fallback.
- **One model, kept in memory.** Offhand loads the weights once and keeps them
  hot.
- **macOS and Windows are equal.** Every platform feature ships on both, and CI
  requires both.
- **You stay in control.** A policy gate stops risky actions until you confirm
  them.

## Platforms

| | macOS | Windows |
|---|---|---|
| UI tree | Accessibility API (`AXUIElement`) | UI Automation |
| Capture | ScreenCaptureKit | Windows.Graphics.Capture |
| Input | CGEvent | SendInput |
| Inference | llama.cpp on Metal | llama.cpp on CUDA or Vulkan |
| Minimum | Apple Silicon, 16 GB | 16 GB RAM, a GPU with 6 GB or more |

## Models

Offhand downloads model weights from Hugging Face on first run. It does not
bundle them.

| Role | Model | ScreenSpot-Pro | Licence |
|---|---|---|---|
| Default | Qwen3.5-4B | 60.3 | Apache-2.0 |
| Small machines | UI-Venus-1.5-2B | 57.7 | Apache-2.0 |
| Option scorer | gua (LoRA on Qwen3.5-4B) | n/a | Apache-2.0 |
| Accuracy baseline | Holo1.5-7B | 57.9 | Apache-2.0 |

Scores come from each model card. The project re-measures all of them in
[`bench/`](bench/). The [thesis](docs/thesis.md) explains each choice.

## Repository layout

```
crates/
  offhand-core       step loop, tier router, task state
  offhand-policy     action risk classes and confirm rules
  offhand-platform   the Platform trait and shared contract tests
  offhand-macos      Accessibility API, CGEvent, ScreenCaptureKit
  offhand-windows    UI Automation, SendInput, Windows.Graphics.Capture
  offhand-infer      the Engine trait and the llama.cpp backend
  offhand-cloud      opt-in cloud tier (feature-gated)
  offhand-cli        the `offhand` command
app/                 Tauri v2 app: command bar, overlay, settings
bench/               latency and accuracy harness
docs/                thesis, architecture, ADRs, roadmap
.claude/             agent personas and skills for AI-assisted development
```

## Build from source

You need Rust 1.94 and Node 22 or later. On macOS, install the Xcode Command
Line Tools. On Windows, install the MSVC Build Tools and WebView2.

```sh
git clone https://github.com/humzakhan/offhand
cd offhand
cargo check --workspace
npm --prefix app install
npm --prefix app run tauri dev
```

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before you open a pull request. If you
use an AI coding agent, point it at [AGENTS.md](AGENTS.md). The repository
ships agent personas and skills in [`.claude/`](.claude/).

## Licence

Offhand is dual-licensed under [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE), at your option. Model weights carry their own
licences. [`models/manifest.toml`](models/manifest.toml) lists each one.
