# Benchmark Claims Cross-Reference

This document maps every public performance claim to its canonical source and defines the rules for quoting benchmark numbers.

## Canonical Source

**[TRUE_BENCHMARKS.md](./TRUE_BENCHMARKS.md)** is the single source of truth for all public performance claims.

All numbers in README.md, PERFORMANCE.md, MIGRATION_FROM_SHARP.md, and ROI_CALCULATOR.md must be traceable to a specific scenario in TRUE_BENCHMARKS.md.

## Public Claims Inventory

| Claim | Location | Source Scenario | Safe to Quote? |
|-------|----------|----------------|----------------|
| "20-30% smaller outputs than sharp" | README.md line 7, line 50 | JPEG −17%, AVIF −40.9% | Yes — range covers measured codec results |
| "1.70x faster AVIF encoding" | PERFORMANCE.md, TRUE_BENCHMARKS.md | PNG → AVIF (no resize, 5000×5000) | Yes — but **workload-specific**, not universal |
| "40.9% smaller AVIF output" | PERFORMANCE.md, TRUE_BENCHMARKS.md | PNG → AVIF (no resize, 5000×5000) | Yes — workload-specific |
| "17.0% smaller JPEG output" | PERFORMANCE.md, TRUE_BENCHMARKS.md | PNG → JPEG (no resize, 5000×5000) | Yes — workload-specific |
| "25% file size reduction" | ROI_CALCULATOR.md (example) | Mid-range estimate between JPEG 17% and AVIF 41% | Yes — labeled as example, not a guarantee |
| Binary size savings (5-9 MB vs 15-21 MB) | PERFORMANCE.md | Measured `.node` binary sizes | Yes — platform-specific |

## Quoting Rules

1. **Always qualify codec and workload.** Never say "lazy-image is faster than sharp" without specifying the format and scenario.
2. **"20-30% smaller" is the safe summary claim.** It spans JPEG (−17%) through AVIF (−41%) and is appropriate for README-level messaging.
3. **Speed claims are workload-specific.** AVIF encoding is faster; WebP encoding is slower. Always cite the specific scenario.
4. **Do not mix resize and no-resize numbers.** Results differ significantly when resize is involved.
5. **Cite the test environment.** TRUE_BENCHMARKS.md specifies the machine, Node version, and library versions used.

## How to Detect Drift

### Automated (CI)

The benchmark regression workflow (`.github/workflows/CI.yml` quality job + `npm run test:bench:verify`) compares actual encoding output against baseline values. If file sizes drift beyond 5% or speed beyond 15%, CI reports it.

### Manual Checklist

When updating README or docs with performance numbers:

1. Run `npm run test:bench:compare` locally to get current numbers
2. Verify the specific scenario exists in TRUE_BENCHMARKS.md
3. Cross-check the claim against this inventory table
4. Update TRUE_BENCHMARKS.md **first**, then update downstream docs

### Files to Check

When TRUE_BENCHMARKS.md is updated, also check:

- `README.md` — summary claims (lines 7, 50)
- `docs/PERFORMANCE.md` — canonical scenarios table
- `docs/MIGRATION_FROM_SHARP.md` — comparison references
- `docs/ROI_CALCULATOR.md` — example reduction percentage

## Version Tracking

TRUE_BENCHMARKS.md line 308 records the lazy-image and sharp versions used. When either dependency updates significantly, re-run benchmarks and update all downstream claims.

Current: lazy-image v0.12.0, sharp v0.34.x
