# Performance Strategy Guardrails

This document defines non-negotiable guardrails for performance changes.

## Principles
- Do not optimize for throughput alone.
- Preserve memory safety and OOM resilience as first-class goals.
- Keep behavior explainable with measurable telemetry.

## Required Checks
1. Throughput:
- `encodeMs` and `totalMs` must be measured for target scenarios.

2. Memory:
- `peakRss` must be tracked with `toBufferWithMetrics`.
- Regressions beyond agreed threshold need explicit approval.

3. Safety:
- Edge-case and bounds tests must pass before merge.
- No known regression on oversized/invalid input handling.

4. Product value:
- Changes must not silently destroy output-size advantages on key formats.
- If tradeoffs are intentional, provide profile/preset-level opt-in.

## Build-profile split
- Node/NAPI release: speed-first (`profile.release`, `opt-level = 3`)
- WASM release: size-first (`profile.release-wasm`, `opt-level = "s"`)

## Quality evaluation axes
- Axis A: compatibility (`lazy-image vs sharp`)
- Axis B: absolute quality (`output vs resized source reference`)

Decisions should use both axes, not one.
