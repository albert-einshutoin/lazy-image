# Resize Semantics (v0.9.x)

Scope: `ImageEngine.resize(width?, height?, fit?)` and resize paths used by `extract`/`processBatch`.

## Fit modes
- `inside` (default): preserve aspect ratio inside the target box. Width/height are treated as maxima.
- `cover`: scale up/down so that both dimensions meet or exceed the target box, then crop center to the exact box.
- `fill`: scale each dimension independently to hit the exact box; aspect ratio may change (no letterboxing/cropping).

## Dimension calculation
- `inside`: aspect-preserving scale; uses `round()` on the non-dominant axis.
- `cover`: aspect-preserving scale; uses `ceil()` on both axes before center crop to the target box.
- `fill`: direct assignment to requested `width`/`height` (after validation).

## Validation and limits
- `width`/`height` must be positive when provided; `0` is rejected with `InvalidResizeDimensions`.
- Global guards apply to the final target size: `MAX_DIMENSION = 32768`, `MAX_PIXELS = 100,000,000`.
- Concurrency guard: `processBatch` and streaming paths cap concurrency at 1024.

## Defaults
- If only `width` is provided, height is scaled with aspect ratio (rounded).
- If only `height` is provided, width is scaled with aspect ratio (rounded).
- If neither is provided, original dimensions are kept.

## Cropping behavior (cover)
- Crop is center-aligned after the up/down-scale.
- Cropping occurs only for `cover`; `inside` never crops.

## Error mapping
- Invalid dimensions ⇒ `UserError / InvalidResizeDimensions`.
- Size exceeds limits ⇒ `ResourceLimit / DimensionExceedsLimit` or `PixelCountExceedsLimit`.
