# Binary Size Comparison (Issue #373)

This document tracks release binary size impact of the optional `avif` feature.

## Measurement Date

- 2026-02-16
- Platform: macOS (Apple Silicon)
- Build target: native release `cdylib` (`lazy-image`)

## Commands

```bash
cargo build --release --no-default-features --features napi
cargo build --release --no-default-features --features "napi,avif"
```

## Results

| Build variant | Bytes | Size |
|---|---:|---:|
| `napi` (without `avif`) | 5,100,352 | 4.86 MiB |
| `napi,avif` | 6,510,912 | 6.21 MiB |
| Delta (`with - without`) | 1,410,560 | 1.35 MiB |
| Delta (%) | 27.66% | - |

## Notes

- `avif` remains enabled by default for backward compatibility.
- Build without AVIF by using `--no-default-features --features napi`.
- In no-AVIF builds, requesting `avif` output returns `UnsupportedFormat`.
