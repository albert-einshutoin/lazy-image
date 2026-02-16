# Benchmark Operations Guide

This document defines how benchmark regression monitoring is operated.

## Workflow

- Workflow file: `.github/workflows/benchmark-regression.yml`
- Triggers:
  - `workflow_dispatch` (manual)
  - `schedule` (weekly, Monday 03:00 UTC)
- Main execution command set:
  - `npm run test:bench:compare`
  - `npm run test:bench:extended`

## Baseline Source

- Baseline file: `.github/benchmarks/baseline.json`
- Result comparison input: `artifacts/benchmark/sharp-comparison.json`
- Result summary outputs:
  - `artifacts/benchmark/regression-summary.md`
  - `artifacts/benchmark/regression-summary.json`

The baseline file is the single source of truth for threshold comparison.

## Threshold Policy

- Speed regression threshold: `+15%` (default)
- Output-size regression threshold: `+5%` (default)
- You can override both thresholds on manual runs (`workflow_dispatch` inputs).

Regression means "current metric value is higher than baseline by more than threshold".

## Failure Handling Policy

- Scheduled runs: `warn` mode (do not fail workflow on regression).
- Manual runs:
  - default is `warn`
  - can be switched to `fail` mode using `fail_on_regression=true`

## Artifacts and Review Flow

- Always upload `benchmark-regression` artifact from workflow run.
- Review `regression-summary.md` in workflow summary first.
- If regressions are expected (intentional change), update `.github/benchmarks/baseline.json` in a dedicated PR.

## Practical Notes

- Benchmark jobs are intentionally separated from required CI gates to avoid noisy PR failures.
- Run manually before releases or after significant image pipeline/perf changes.
