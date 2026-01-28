# Quality Value Semantics (JPEG / WebP / AVIF / PNG)

This document clarifies what a `quality` number means for each encoder and how to pick values consistently.

## TL;DR
- The same 0–100 number maps to shared bands (High / Balanced / Fast / Fastest) across formats (see `docs/QUALITY_EFFORT_SPEED_MAPPING.md`).
- Default qualities are tuned per format: **JPEG 85**, **WebP 80**, **AVIF 60**. PNG ignores `quality`.
- Values are clamped to 0–100. Passing `undefined` uses the format default.

## Format Semantics
| Format | Uses `quality`? | Encoder knobs affected | Default | Notes |
| --- | --- | --- | --- | --- |
| JPEG (mozjpeg) | Yes | Quantization tables; smoothing; chroma subsampling | 85 | Higher = fewer artifacts, larger files. `fast_mode` controls speed profile, not quality. |
| WebP (libwebp) | Yes | Quantization factor + filter strength/sharpness + SNS | 80 | `method` is fixed (4) for sharp parity; quality still drives quantizer and filtering. |
| AVIF (libavif) | Yes | Quantizer + speed trade-off (via shared bands) | 60 | Lower numbers can still look good; encoder speed set by band (6–9). |
| PNG | No (ignored) | Lossless; uses fixed compression level | n/a | `quality` is accepted for API compatibility but has no effect. |

## Cross-format quality guidance
Use these ranges when you want similar subjective quality across formats:

| Intent | JPEG | WebP | AVIF | Notes |
| --- | --- | --- | --- | --- |
| High detail | 90–95 | 85–90 | 70–80 | Preserve fine textures; larger files. |
| Default/general | 82–88 | 78–82 | 60–70 | Matches project defaults. |
| Fast delivery | 70–80 | 68–75 | 50–60 | Prioritize throughput and size. |
| Lowest latency | 50–65 | 50–65 | 35–55 | Use when speed trumps fidelity. |

## API expectations
- `quality` range: **0–100**, clamped. Values <0 → 0, >100 → 100.
- If you need deterministic size/latency, prefer sticking to the shared bands rather than arbitrary single numbers.
- For PNG outputs, omit the quality argument to signal intent clearly (it is ignored either way).

## When to adjust per format
- JPEG: Raise quality for gradient-heavy photos to avoid banding; consider `fast_mode: true` if latency-critical.
- WebP: Slightly lower quality can still hold detail; filtering is already tuned for web defaults.
- AVIF: Increasing quality increases encode time sharply; consider raising only when targeting hero images.
