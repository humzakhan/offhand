# bench

The benchmark harness measures latency and accuracy for each candidate model
and engine. Its results decide ADR-0002 (model and engine).

## What it measures

| Metric | Definition |
|---|---|
| Encode | Time to encode the screenshot into visual tokens |
| TTFT | Time to the first output token (encode plus prefill) |
| Step | Wall time for one full T1 or T2 decision |
| Peak memory | Resident memory of the inference process |
| ScreenSpot-Pro | Grounding accuracy on the public benchmark |
| Task success | Pass rate on the tasks in `tasks/macos` and `tasks/windows` |

The harness runs each metric at three crop sizes: full screen, active window,
and target region.

## Layout

```
bench/
  tasks/macos/     50 real-app tasks, one YAML file each
  tasks/windows/   50 real-app tasks, one YAML file each
  results/         one CSV per machine, model, and commit
```

## Status

The harness is not written yet. It is the week-2 item in
[`docs/roadmap.md`](../docs/roadmap.md). It uses Python with `uv` so that it
can also compare llama.cpp with MLX on macOS.
