# Pipeline Semantics (v0.9.x)

Scope: overall pipeline execution rules across operations such as resize, crop, rotate, and pixel-space adjustments.

## Operation ordering and fusion
- Operations are applied in the order they are queued on `ImageEngine`.
- Before execution, the queue is passed through `optimize_ops` (deterministic, no reordering across different op types):
  - Consecutive `resize` ops with the same `fit` are collapsed into a single resize; the last explicit `width`/`height` wins and the other side is recomputed from aspect ratio.
  - `resize → crop` is fused into `extract` when `fit` is **not** `cover`, preserving the semantics of "resize first, then crop" while avoiding an intermediate buffer. `cover` stays unfused to keep the explicit center-crop step visible.
  - `crop → resize` with `fit=inside` keeps the user order; the final resize dimensions are precomputed from the cropped size so rounding depends on the cropped rectangle, not the original image.
  - No other reordering occurs; every subsequent op (rotate/flip/adjust/etc.) runs after the result of any fused pair.

## Boundary and rounding rules for resize + crop compositions
- `fit=inside`: the non-dominant axis uses `round()` (ties away from zero). Example: 100×49 cropped then resized to `width=50` yields `50×25` because `49 * 0.5 = 24.5 → 25`.
- `fit=cover`: both axes use `ceil()` before center-cropping to the target box. When the centered offset is fractional, integer division rounds down (`(resize_dim - target_dim) / 2`).
- `fit=fill`: requested integer dimensions are used after validation.
- Fused `extract` (resize→crop):
  - Crop coordinates are validated against the resized frame `(resize_w, resize_h)`; a zero `crop_width`/`crop_height` or out-of-bounds rectangle is rejected.
  - The crop window is mapped back to source space with double-precision scale factors and clamped to the source image; the output size is exactly `crop_width × crop_height`.
- `crop → resize`: resize calculations use the cropped width/height as the source dimensions; rounding therefore reflects the cropped boundaries instead of the original image.
- All paths reject zero-size resizes/crops and never sample outside the original image bounds.
