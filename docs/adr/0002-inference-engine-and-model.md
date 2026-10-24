# ADR-0002: Inference engine and default model

- Status: Proposed
- Date: 2026-10-24

## Context

Offhand needs one inference engine that serves a vision model on Metal, CUDA,
and Vulkan. It needs a default model that fits a 16 GB machine.

## Proposal

- Engine: llama.cpp with `libmtmd`, through the `llama-cpp-2` crate behind the
  `Engine` trait. `llama-cpp-4` and `mistral.rs` are fallbacks.
- Default model: Qwen3.5-4B. Small-machine model: UI-Venus-1.5-2B.

## Acceptance condition

The week-2 benchmark must show that T1 completes in about 500 ms or less on
the reference Mac. If the 4B model fails this condition, UI-Venus-1.5-2B
becomes the default.

## Consequences

- The project pins one llama.cpp commit because the `libmtmd` API changes
  often.
- MLX stays out of the build unless the benchmark shows a large gap on macOS.
