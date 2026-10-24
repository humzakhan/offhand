# ADR-0003: Tiered step loop

- Status: Accepted
- Date: 2026-10-24

## Context

A screenshot-to-model loop takes 17 to 35 seconds per step with a 7B model on
an M5 Mac. Accessibility APIs can resolve many targets with no model call.

## Decision

Use four tiers in a fixed order: T0 accessibility tree, T1 option choice from
model logits, T2 coordinate grounding, T3 opt-in cloud. Each tier runs only
if the tier before it cannot resolve the target. Every action passes through
`offhand-policy`.

## Consequences

- Most steps in apps with good accessibility support need no model call.
- Apps with custom-drawn UI fall through to T1 or T2.
- The policy gate is a single point that every action must pass.
