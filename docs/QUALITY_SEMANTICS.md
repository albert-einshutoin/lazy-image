# Choosing quality

Quality controls encoder settings, not a universal perceptual score. The same
number does not promise equal quality, size or latency across formats or libraries.

| Output | Default quality | Meaning |
|---|---|---|
| JPEG | 85 | Lossy encoding; increasing quality generally preserves more detail |
| WebP | 80 | Lossy quality and related encoder settings |
| AVIF | 60 | Quantization and speed settings depend on the quality band |
| PNG | Ignored | Lossless output; omit quality to make the intent clear |

When provided, quality must be an integer from 1 through 100. `undefined` selects
the format default. These are `ImageEngine` defaults; presets and the compiler
have their own documented options. See [API](./API.md).

## Select a value

Start with the format default, inspect representative photos, UI graphics and
transparent inputs, then measure size and time. Check fine detail, gradients,
alpha and color behavior. A fixed quality band does not make size or latency
deterministic. Use [performance evidence](./PERFORMANCE.md) for comparison scope.

If a delivery size is mandatory, use a target-byte output and check `budgetMet`,
or declare a compiler artifact budget that must pass before publication. Meeting
a byte cap alone does not guarantee acceptable visual quality.

For implementation details, see [encoder parameter mapping](./QUALITY_EFFORT_SPEED_MAPPING.md).
