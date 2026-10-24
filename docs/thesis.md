# Thesis

This document records the research thesis behind Offhand. It covers the model
choice, the agent loop, the inference engine, the local and cloud trade-off,
and the app shell. The research date is 2026-10-24. Source links are in
[`research/README.md`](research/README.md).

## Claim

A fast, offline computer-use agent for macOS and Windows is feasible if the
model is the last resort. The agent acts through the accessibility tree when
it can. When it cannot, a small local model picks from a list of candidate
elements. Coordinate grounding comes after that. A cloud model is an optional
tier for planning and recovery. It is never a requirement.

## Why a screenshot-to-model loop is too slow

MacPaw measured UI-TARS-1.5-7B on an M5 Mac with 24 GB of memory. One step
took 17 to 35 seconds, and the model took about 9 seconds to load. Most of
the cost of a vision model step is image encoding and prefill, not output
tokens. Prefill was 86% of inference time in one LLaVA-NeXT 7B study.
Fewer visual tokens and a cached image prefix give the largest savings.

## Model choice

| Role | Model | ScreenSpot-Pro | Licence |
|---|---|---|---|
| Default | Qwen3.5-4B | 60.3 | Apache-2.0 |
| Small machines | UI-Venus-1.5-2B | 57.7 | Apache-2.0 |
| Option scorer | gua (LoRA on Qwen3.5-4B) | n/a | Apache-2.0 |
| Accuracy baseline | Holo1.5-7B | 57.9 | Apache-2.0 |

Qwen3.5-4B has the highest published ScreenSpot-Pro score under 5B
parameters. It also scores 35.6 on OSWorld-Verified. It has MLX and GGUF
builds. The gua option scorer is a LoRA on the same base, so one copy of the
weights serves T1 and T2.

These models were rejected or deferred:

| Model | Reason |
|---|---|
| UI-TARS-1.5-7B | 17 to 35 s per step on an M5 |
| FastVLM | Research-only licence; no GUI benchmarks |
| OmniParser v2 icon detector | AGPL-3.0 |
| DeskForge-Qwen3.5-4B | 49.02 on ScreenSpot-Pro, below its own base model on Qwen's card |
| Northstar CUA Fast 1.6 | No published latency or grounding scores |

All scores come from model cards and use different harnesses. The project
re-measures every candidate in `bench/` before ADR-0002 is accepted.

## Agent loop

| Tier | Input | Output | Target |
|---|---|---|---|
| T0 | Accessibility tree, shortcut table | Direct action | < 50 ms |
| T1 | Cropped frame, numbered candidates | Option index from logits | < 400 ms |
| T2 | Cropped frame, task | (x, y) | < 1.5 s |
| T3 | Full context | Plan or recovery step | Network bound |

The targets are design goals, not measurements. The benchmark sets the real
pass and fail thresholds.

Published results support a hybrid input. EntWorld found that a screenshot
plus the accessibility tree was the strongest input. A tree alone was not
enough on legacy enterprise apps. Screenshots alone were the weakest.

## Inference engine

llama.cpp with its multimodal library (`libmtmd`) is the only engine that
serves vision models on Metal, CUDA, and Vulkan from one MIT-licensed C
library. Offhand uses it on both platforms. MLX is a possible faster engine on
macOS if the benchmark shows a real gap. The `libmtmd` API changes often, so
the project pins a llama.cpp commit.

Windows NPUs are a stretch goal. The only data point is Qwen3-VL-2B on a
Snapdragon X Elite: 0.16 to 0.74 s to the first token at 512 context, and
about 14 s at 4096 context.

## Local and cloud

Local is the default because it keeps screen content on the device and has no
per-step cost. The cloud tier is opt-in and uses the user's own API key.

| Option | Fit |
|---|---|
| Azure AI Personalizer ("Decision") | Retired on 1 October 2026; it was never a vision model |
| Microsoft Fara-7B and Fara1.5 | Open weights (MIT); candidates for the local tier |
| Cloudflare `clef` and `clef-flash` | Return option probabilities from images; no coordinates. Fit for step verification |
| Cloudflare Moondream 3.1 | Returns points and boxes; no UI benchmarks |
| Claude Haiku 5.5 | $0.10 per million input tokens; low-cost planner |
| Claude Sonnet 5.5 | Recovery from stuck states |
| Gemini free tier | Google can use inputs to improve products; the app must warn the user |

## App shell

Offhand uses one Rust workspace and a Tauri v2 app with a React UI. This
gives one language for the core and one UI codebase. Tauri documents
click-through windows (`set_ignore_cursor_events`) and capture exclusion
(`set_content_protected`). The project tests both on macOS and Windows in
week 1. Native overlay windows from Rust are the fallback.

## Distribution

- macOS needs an Apple Developer ID ($99 per year). Homebrew 5 deprecated
  unsigned casks. Ad hoc signatures reset Accessibility and Screen Recording
  grants on every build.
- Windows signing is free through SignPath Foundation for OSI-licensed
  projects.
- Model weights download from Hugging Face on first run.

## Condition that changes the thesis

If T1 cannot complete in about 500 ms with a 4B model on a 16 GB Mac,
UI-Venus-1.5-2B becomes the default model.
