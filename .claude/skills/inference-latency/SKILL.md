---
name: inference-latency
description: Use when you change capture, prompts, image handling, or anything in offhand-infer. Covers the latency techniques and the required before-and-after benchmark table.
---

# Inference latency

Offhand targets under 400 ms for T1 and under 1.5 s for T2. A 7B model in a plain screenshot loop takes 17 to 35 seconds per step. Each rule below removes a cost from that loop.

## Rules

1. **Keep the weights resident.** Load the model once at start-up. Never reload per step. Load cost is seconds.
2. **Crop to the window or region.** Capture one window, then crop to the area that matters. Fewer pixels mean fewer visual tokens.
3. **Cap visual tokens.** Set a hard max on image tokens in the `mtmd` config. Record the cap with each bench run.
4. **Reuse the KV cache of the system prompt.** Keep the system prompt byte-identical across steps so its cache entries stay valid. Put changing text after it.
5. **Cache image prefixes by hash.** Hash the cropped image. If the hash matches, skip the vision encoder.
6. **Score options from logits.** For T1, list the options as numbered choices. Read the logits of the option tokens and take the argmax. Do not decode text.
7. **Do not run the model at all** when the tree resolves the target. See `accessibility-first`.

```rust
// Score N options with one forward pass. No sampling loop.
let logits = ctx.last_logits();
let best = option_token_ids
    .iter()
    .enumerate()
    .max_by(|a, b| logits[*a.1].total_cmp(&logits[*b.1]))
    .map(|(i, _)| i);
```

## Measure before and after

Any change to capture, prompts, or inference needs a before-and-after run from `bench/`.

1. Run the base commit: `cargo xtask bench`.
2. Apply the change. Run again on the same machine.
3. Put this table in the pull request.

```
| Tier | Metric | Before | After | Target |
|---|---|---|---|---|
| T1 | p50 ms | 0 | 0 | 400 |
| T1 | p95 ms | 0 | 0 | 400 |
| T2 | p50 ms | 0 | 0 | 1500 |
```

State the machine, the model, the crop size, and the commit under the table. Report cold start and warm runs apart.

## Checklist

- [ ] Weights load once
- [ ] Capture is cropped, and the crop size is recorded
- [ ] Visual token cap is set
- [ ] System prompt is stable, so its KV cache is reused
- [ ] Image encodings are cached by hash
- [ ] T1 reads logits and does not decode text
- [ ] Before and after table is in the PR
- [ ] A new model is in `models/manifest.toml` with SHA-256 and licence
