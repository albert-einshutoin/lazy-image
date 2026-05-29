# Benchmark Claims Cross-Reference

This document maps every public performance claim to its canonical source, classifies claims by safety level, and defines a repeatable process to prevent drift.

## Canonical Source

**[TRUE_BENCHMARKS.md](./TRUE_BENCHMARKS.md)** is the single source of truth for all public performance claims.

All numbers in README.md, PERFORMANCE.md, MIGRATION_FROM_SHARP.md, and ROI_CALCULATOR.md must be traceable to a specific scenario in TRUE_BENCHMARKS.md.

**Why a single source?** Multiple docs quoting independently-measured numbers inevitably diverge. By requiring every public claim to trace back to TRUE_BENCHMARKS.md, updates flow in one direction and inconsistencies surface as missing cross-references rather than contradictory numbers.

## Claim Classification

### Public-safe claims

These may appear in README, marketing, and migration guides **without qualification beyond what is already stated**:

| Claim | Location | Source Scenario | Notes |
|-------|----------|----------------|-------|
| "17-20% smaller JPEG outputs than sharp" | README.md, PERFORMANCE.md | PNG -> JPEG no-resize -17.0%; PNG -> JPEG resize 800px -20.0% | JPEG-specific; do not generalize to AVIF/WebP |
| "18% file size reduction" | ROI_CALCULATOR.md (example) | JPEG-oriented worked example | Labeled as example, not a guarantee |
| Binary size savings (5-9 MB vs 15-21 MB) | PERFORMANCE.md | Measured `.node` binary sizes | Platform-specific, factual |

### Workload-specific claims

These **must always include the codec, input format, and scenario** when quoted:

| Claim | Location | Source Scenario | Required qualification |
|-------|----------|----------------|----------------------|
| "17.0% smaller JPEG output" | PERFORMANCE.md, TRUE_BENCHMARKS.md | PNG -> JPEG (no resize, 5000×5000) | Must cite format + input size |
| "20.0% smaller JPEG output" | PERFORMANCE.md, TRUE_BENCHMARKS.md | PNG -> JPEG (resize 800px, 5000×5000 input) | Must cite format + resize context |
| "AVIF is slower and larger in current canonical PNG -> AVIF benchmarks" | PERFORMANCE.md, TRUE_BENCHMARKS.md | PNG -> AVIF no-resize and resize 800px | Must cite exact scenario; do not turn into a universal AVIF claim |
| "WebP is slower and similar/larger in current canonical PNG -> WebP benchmarks" | PERFORMANCE.md, TRUE_BENCHMARKS.md | PNG -> WebP no-resize and resize 800px | Must cite exact scenario; do not turn into a universal WebP claim |

**Rule of thumb:** If removing the scenario context would mislead a reader into thinking the claim is universal, it is workload-specific.

## Quoting Rules

1. **Always qualify codec and workload.** Never say "lazy-image is faster than sharp" without specifying the format and scenario.
2. **"17-20% smaller JPEG" is the safe summary claim.** It applies only to the canonical PNG -> JPEG scenarios currently measured.
3. **Speed claims are workload-specific.** Current JPEG resize was faster, JPEG no-resize was slightly slower, and AVIF/WebP favored sharp in the measured scenarios. Always cite the specific scenario.
4. **Do not mix resize and no-resize numbers.** Results differ significantly when resize is involved.
5. **Cite the test environment.** TRUE_BENCHMARKS.md specifies the machine, Node version, and library versions used.

## Cross-Reference Rules

These rules prevent drift between documents:

1. **Upstream-first.** Update TRUE_BENCHMARKS.md before any downstream doc. Never update README or PERFORMANCE.md with numbers that are not yet in TRUE_BENCHMARKS.md.
2. **README uses qualified ranges only.** README may state "17-20% smaller JPEG" but must not imply all codecs are smaller or faster.
3. **Mandatory qualification.** Workload-specific claims must include format, input dimensions, and whether resize was applied. Omitting any of these is a documentation bug.
4. **Version-bump trigger.** When lazy-image, sharp, or any core encoder dependency (mozjpeg, libavif, libwebp) is updated, re-run `npm run test:bench`, `node --expose-gc test/benchmarks/convert-only.bench.js`, and `npm run test:bench:extended` before updating TRUE_BENCHMARKS.md.
5. **Atomic updates.** When a benchmark number changes, update all downstream files in the same PR. Partial updates create drift windows.

## How to Detect Drift

### Automated (CI)

`npm run test:bench:verify` and benchmark smoke checks ensure the benchmark scripts still execute. Numeric public-claim drift is currently governed by this document and manual upstream-first updates; broader baseline automation is tracked separately.

### Local Verification

```bash
# Run the full benchmark comparison
npm run test:bench:compare

# Run no-resize conversion numbers cited in TRUE_BENCHMARKS.md
node --expose-gc test/benchmarks/convert-only.bench.js

# Run extended AVIF/WebP/JPEG, memory, and cold-start snapshots
npm run test:bench:extended

# Verify README claims match actual results
npm run test:bench:verify

# Quick smoke test for size/quality guards
npm run test:bench:smoke
```

### Update Checklist

Use this checklist when updating any performance claim:

**Before updating:**
- [ ] Run `npm run test:bench`, `node --expose-gc test/benchmarks/convert-only.bench.js`, and `npm run test:bench:extended` locally and record current numbers
- [ ] Verify the specific scenario exists in TRUE_BENCHMARKS.md
- [ ] Cross-check the claim against the inventory tables above

**During the update:**
- [ ] Update TRUE_BENCHMARKS.md **first** with new numbers and test environment
- [ ] Update downstream docs (PERFORMANCE.md, README.md, etc.) in the **same PR**
- [ ] Classify each new claim as public-safe or workload-specific
- [ ] Ensure workload-specific claims include format, input size, and resize context

**After updating:**
- [ ] Run `npm run test:bench:verify` to confirm benchmark verification still executes
- [ ] Verify CI passes the benchmark and test jobs
- [ ] Update the version tracking section below if library versions changed

### Files to Check

When TRUE_BENCHMARKS.md is updated, also check:

| File | What to verify |
|------|---------------|
| `README.md` | Summary claims containing "17-20% smaller JPEG" stay within measured range (grep, don't rely on line numbers) |
| `docs/PERFORMANCE.md` | Canonical scenarios table matches TRUE_BENCHMARKS.md |
| `docs/MIGRATION_FROM_SHARP.md` | Comparison references are current |
| `docs/ROI_CALCULATOR.md` | Example reduction percentage is plausible |
| `docs/BENCHMARK_RESULTS.md` | Historical snapshots are not confused with current numbers |
| `docs/BENCHMARK_OPERATIONS.md` | Operation-level benchmarks are consistent |
| `test/benchmarks/readme-verification.bench.js` | Script still executes and any reintroduced expected values match TRUE_BENCHMARKS.md |

## Version Tracking

The `*Last updated:*` footer at the bottom of TRUE_BENCHMARKS.md records the lazy-image and sharp versions used. When either dependency updates significantly, re-run benchmarks and update all downstream claims.

Current: lazy-image v0.15.0, Node.js v24.2.0, sharp v0.34.5 (benchmarked 2026-05-29)
