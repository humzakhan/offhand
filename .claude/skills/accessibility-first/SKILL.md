---
name: accessibility-first
description: Use when you add or change how Offhand finds a target or decides to call a model. Covers the tier order T0 to T3, when escalation is allowed, and tree caching.
---

# Accessibility first

The accessibility tree is faster and more exact than vision. Use vision only when the tree cannot answer.

## Tier order

| Tier | Method | Target latency | Use when |
|---|---|---|---|
| T0 | Accessibility tree and shortcuts | under 50 ms | The tree names the target |
| T1 | Crop window, model picks option N from logits | under 400 ms | The tree has candidates but is ambiguous |
| T2 | Model grounds to (x, y) | under 1.5 s | The target is not in the tree |
| T3 | Cloud model, opt-in | n/a | Stuck, or a long-horizon plan |

## Rules

1. Never call the model when the tree resolves the target. Do not add a model call where a tree query can answer.
2. Escalate one tier at a time. Do not jump from T0 to T2.
3. Escalate only when the lower tier returns no match, more than one match you cannot rank, or a tree that is empty or stale.
4. T3 runs only when the `cloud` feature is built in and the user turned it on.
5. Log the tier that resolved each step. The bench uses this to report the tier mix.

## Cache the tree

Read the tree once per window. Keep it until a window-change event arrives: `AXObserver` on macOS, UIA events on Windows. Do not re-walk the tree on every step.

```rust
fn tree(&mut self, win: WindowId) -> Result<&Tree, PlatformError> {
    if self.dirty.take(&win) || !self.cache.contains_key(&win) {
        self.cache.insert(win, self.platform.read_tree(win)?);
    }
    Ok(&self.cache[&win])
}
```

Mark the cache dirty on any event that changes the window: focus, value, layout, or children.

## Empty trees

Chromium and Electron windows expose a tree only when accessibility is on. An empty tree for a known browser window means "turn accessibility on", not "go to T2". Try that first. Games and canvas apps are the real T2 cases.

## Query before you ask the model

Match by role, then by name, then by position. Use fuzzy name match only after exact match fails. Try keyboard shortcuts before clicks when the app defines one.

## Checklist

- [ ] The tree was tried and the miss is logged
- [ ] Escalation is one tier at a time
- [ ] The tree is cached until a window-change event
- [ ] No model call where a tree query works
- [ ] Empty Chromium or Electron trees get the accessibility step first
- [ ] The resolving tier is logged
