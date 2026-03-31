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
| "20-30% smaller outputs than sharp" | README.md | JPEG −17%, AVIF −40.9% | Range covers measured codec results |
| "25% file size reduction" | ROI_CALCULATOR.md (example) | Mid-range estimate | Labeled as example, not a guarantee |
| Binary size savings (5-9 MB vs 15-21 MB) | PERFORMANCE.md | Measured `.node` binary sizes | Platform-specific, factual |

### Workload-specific claims

These **must always include the codec, input format, and scenario** when quoted:

| Claim | Location | Source Scenario | Required qualification |
|-------|----------|----------------|----------------------|
| "1.70x faster AVIF encoding" | PERFORMANCE.md, TRUE_BENCHMARKS.md | PNG → AVIF (no resize, 5000×5000) | Must cite format + input size |
| "40.9% smaller AVIF output" | PERFORMANCE.md, TRUE_BENCHMARKS.md | PNG → AVIF (no resize, 5000×5000) | Must cite format + input size |
| "17.0% smaller JPEG output" | PERFORMANCE.md, TRUE_BENCHMARKS.md | PNG → JPEG (no resize, 5000×5000) | Must cite format + input size |

**Rule of thumb:** If removing the scenario context would mislead a reader into thinking the claim is universal, it is workload-specific.

## Quoting Rules

1. **Always qualify codec and workload.** Never say "lazy-image is faster than sharp" without specifying the format and scenario.
2. **"20-30% smaller" is the safe summary claim.** It spans JPEG (−17%) through AVIF (−41%) and is appropriate for README-level messaging.
3. **Speed claims are workload-specific.** AVIF encoding is faster; WebP encoding is slower. Always cite the specific scenario.
4. **Do not mix resize and no-resize numbers.** Results differ significantly when resize is involved.
5. **Cite the test environment.** TRUE_BENCHMARKS.md specifies the machine, Node version, and library versions used.

## Cross-Reference Rules

These rules prevent drift between documents:

1. **Upstream-first.** Update TRUE_BENCHMARKS.md before any downstream doc. Never update README or PERFORMANCE.md with numbers that are not yet in TRUE_BENCHMARKS.md.
2. **README uses ranges only.** README may state "20-30% smaller" but must not cite specific percentages (e.g. "40.9%") — those belong in PERFORMANCE.md with a link to the source scenario.
3. **Mandatory qualification.** Workload-specific claims must include format, input dimensions, and whether resize was applied. Omitting any of these is a documentation bug.
4. **Version-bump trigger.** When lazy-image or sharp is updated to a new minor/major version, re-run `npm run test:bench:compare` and update TRUE_BENCHMARKS.md before merging the version bump.
5. **Atomic updates.** When a benchmark number changes, update all downstream files in the same PR. Partial updates create drift windows.

## How to Detect Drift

### Automated (CI)

The benchmark regression workflow (`.github/workflows/CI.yml` quality job + `npm run test:bench:verify`) compares actual encoding output against baseline values. If file sizes drift beyond 5% or speed beyond 15%, CI reports it.

### Local Verification

```bash
# Run the full benchmark comparison
npm run test:bench:compare

# Verify README claims match actual results
npm run test:bench:verify

# Quick smoke test for size/quality guards
npm run test:bench:smoke
```

### Update Checklist

Use this checklist when updating any performance claim:

**Before updating:**
- [ ] Run `npm run test:bench:compare` locally and record current numbers
- [ ] Verify the specific scenario exists in TRUE_BENCHMARKS.md
- [ ] Cross-check the claim against the inventory tables above

**During the update:**
- [ ] Update TRUE_BENCHMARKS.md **first** with new numbers and test environment
- [ ] Update downstream docs (PERFORMANCE.md, README.md, etc.) in the **same PR**
- [ ] Classify each new claim as public-safe or workload-specific
- [ ] Ensure workload-specific claims include format, input size, and resize context

**After updating:**
- [ ] Run `npm run test:bench:verify` to confirm README/doc claims are consistent
- [ ] Verify CI passes the quality regression job
- [ ] Update the version tracking section below if library versions changed

### Files to Check

When TRUE_BENCHMARKS.md is updated, also check:

| File | What to verify |
|------|---------------|
| `README.md` | Summary claims (lines 7, 50) stay within measured range |
| `docs/PERFORMANCE.md` | Canonical scenarios table matches TRUE_BENCHMARKS.md |
| `docs/MIGRATION_FROM_SHARP.md` | Comparison references are current |
| `docs/ROI_CALCULATOR.md` | Example reduction percentage is plausible |
| `docs/BENCHMARK_RESULTS.md` | Historical snapshots are not confused with current numbers |
| `docs/BENCHMARK_OPERATIONS.md` | Operation-level benchmarks are consistent |
| `test/benchmarks/readme-verification.bench.js` | Expected values match TRUE_BENCHMARKS.md |

## Version Tracking

TRUE_BENCHMARKS.md line 308 records the lazy-image and sharp versions used. When either dependency updates significantly, re-run benchmarks and update all downstream claims.

Current: lazy-image v0.12.0, sharp v0.34.x
