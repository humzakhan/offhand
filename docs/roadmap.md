# Roadmap

This roadmap lists the milestones to version 0.1. Each item ships on macOS and
Windows unless the item says otherwise.

| Week | Milestone | Exit check |
|---|---|---|
| 1 | Spikes: overlay on both platforms, llama-cpp-2 vision call, tree read with AX and UIA | Each spike has a short report in `docs/research/` |
| 2 | Benchmark harness; accept ADR-0002 (model and engine) | Results CSVs in `bench/results/` |
| 3–4 | T0 on both platforms, action policy, CLI | Contract tests pass on both CI legs |
| 5–6 | T1 and T2 inference, crop, image cache | T1 meets its latency target on the reference Mac |
| 7 | App: command bar, overlay, first-run model download | Manual test on both platforms |
| 8 | Signing (Developer ID, SignPath) and release v0.1 | Signed `.dmg` and `.msi` on GitHub Releases |
| 9 | T3 cloud tier, opt-in | Cloud disabled builds open no sockets |

## Not in v0.1

- Linux support.
- NPU inference.
- Task recording and replay.
