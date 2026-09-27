# Quality / Effort / Speed Mapping

This document freezes how `QualitySettings` converts a quality value (1-100) into encoder parameters. The quality bands are shared across formats so the same number yields the same banded behavior.

## Shared Quality Bands

| Band | Quality range | Goal | AVIF speed | Notes |
| --- | --- | --- | --- | --- |
| High | 85-100 | Visual fidelity first | 6 | Lowest noise, best detail |
| Balanced | 70-84 | Quality/latency balance | 7 | Default “high quality” tier |
| Fast | 50-69 | Size/throughput oriented | 8 | Keep quality reasonable while speeding up |
| Fastest | 1-49 | Lowest latency | 9 | Prioritize speed over fidelity |

## Format-specific mapping

### JPEG (mozjpeg)
- `fast_mode: false` (default) enables aggressive quality optimizations.
- `fast_mode: true` selects the faster JPEG configuration; it does not guarantee another engine's speed.
- Smoothing: 90+ → 0, 70-89 → 5, 60-69 → 10, 1-59 → 18.

### WebP
- `method: 4`, `pass: 1`, `preprocessing: 0` are fixed (sharp-equivalent).
- `sns_strength`: High=50 / Balanced=70 / Fast & Fastest=80.
- `filter_strength`: >=80 -> 20, 60-79 -> 30, <=59 -> 40 (keeps sharp-style cutoffs).
- `filter_sharpness`: High=2 / others=0.

### AVIF (libavif speed)
- Speed ranges 0 (slowest/best) to 10 (fastest/worst).
- Mapping: High=6, Balanced=7, Fast=8, Fastest=9.

## Choosing quality

These bands describe encoder parameter selection, not perceptual equivalence or
guaranteed latency. See [quality guidance](./QUALITY_SEMANTICS.md) for user choices.
