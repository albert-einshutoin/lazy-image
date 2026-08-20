# Selective test CI

## Purpose and safety contract

Pull requests use change-impact analysis to run the smallest test set that can
be selected with evidence. The goal is to reduce runner time and compute cost
without weakening failure detection. The invariant is deliberately simple:

```text
safe classification  -> selective tests + always-on smoke test
uncertain or failed   -> complete test suite
```

The planner never treats an analysis error as permission to skip tests. Pushes
to `main` and `release/**`, version tags, manual full validation, and the daily
03:17 JST schedule continue to use `.github/workflows/CI.yml` and its complete
cross-platform matrix.

The orchestration in `ci/` is plain Node.js and Git. It does not import a CI
SDK, test framework, monorepo tool, or build-tool API. GitHub Actions only wires
the portable planner and runner into this repository.

## How impact is determined

`ci/plan-tests.js` resolves the merge base of the latest target revision and
the PR head, then reads `git diff --name-status -z --find-renames
--find-copies`. Added, modified, deleted, renamed, and copied paths are
normalized. Deleted paths take part in classification but are never passed to
a test command as if they still existed. If a revision, merge base, diff, or
configuration cannot be read—even after shallow-clone fetch attempts—the plan
is `full`.

The planner applies these stages in order:

1. Match centralized high-risk rules in `ci/config/full-test-rules.json`.
2. Detect projects from manifests through `ci/adapters/`.
3. Map changed paths to modules using `ci/config/module-mappings.json`.
4. Walk reverse `dependsOn` edges to include every upstream consumer.
5. Add the mapped unit, integration, and E2E targets.
6. Add directly changed test files.
7. Add `ci/config/smoke-tests.json` unconditionally.
8. Reject unclassified files, missing targets, and zero-target plans.

The explicit dependency graph complements path matching. For example, a
change in Rust memory management affects pipeline, tasks, API, JavaScript
helpers, and streaming consumers, so their mapped tests are selected as well.
This repository does not currently have a product E2E suite; the schema and
runner support `e2eTests` targets when one is introduced.

The result is written as JSON in `artifacts/ci/test-plan.json`, including base
and head revisions, changed files, detected adapters, affected projects and
modules, categorized targets, strategy, and fallback reason.

## Supported adapters and projects

The adapter registry currently detects JavaScript/TypeScript (`package.json`),
Rust (`Cargo.toml`), Python (`pyproject.toml`, `requirements.txt`, `setup.py`),
and Go (`go.mod`). lazy-image has tested mappings and commands for its root
Node/Rust project, Rust fuzz crate, and JavaScript Wasm workspace. Detection of
a new language is not considered proof that its tests can be selected: until
its modules are mapped, its changes fall back to the full suite.

Each file in `ci/adapters/` declares manifest names, full-test commands, build
and static-analysis commands, cache candidates, additional high-risk patterns,
the related-test selection method, target command generation, and target
validation. Mapped targets may use `<adapter>::<target>`; unprefixed targets use
`defaultTestAdapter` from project settings. Language-specific command and
discovery behavior must stay in the adapter rather than the common runner. To
add an adapter:

1. Add `ci/adapters/<language>.js` with the same exported fields.
2. Register it in `ci/lib/adapters.js`.
3. Add its source paths, dependency edges, and test targets to
   `ci/config/module-mappings.json`.
4. Add dependency, compiler, build, test, CI, container, schema, and global
   contract files to `ci/config/full-test-rules.json`.
5. Add planner tests proving both a selective case and every uncertain
   fallback case.

## Full-suite fallback

High-risk rules include dependency manifests and locks, compiler/native build
configuration, CI and selective-test configuration, public API loaders and
types, shared Rust crate/engine/error/operation contracts, shared test helpers
and fixtures, golden baselines, containers, and environment contracts. An
unclassified source or important file also triggers full testing.

Runtime validation repeats the safety check. A malformed plan, nonexistent
selected test, unsupported target, or zero executable targets switches to
`npm test`. A test failure remains a failure; cache restore failures and test
runner failures cannot produce a green result.

## Local use and manual full validation

Reproduce a PR decision against the current target branch:

```bash
git fetch origin main
npm run ci:plan-tests -- \
  --base origin/main \
  --head HEAD \
  --output artifacts/ci/test-plan.json
npm run ci:run-tests -- \
  --plan artifacts/ci/test-plan.json \
  --summary artifacts/ci/local-summary.json
```

Run complete validation locally with:

```bash
npm run build
npm test
```

The GitHub `CI` workflow can also be dispatched manually with
`full_validation=true` (the default). Its full matrix is the release-grade
validation path, and this mode does not enter the publish job. A manual publish
requires deliberately disabling `full_validation` and `dry_run`.

## Logs, history, comparison, and cost

Planner logs show revisions, adapters, changed files, affected projects and
modules, strategy, fallback reason, categories, and target counts. Runner logs
show each command plus success/failure/skip counts, wall time, and summed
compute time. JSON summaries and JSONL history are uploaded as workflow
artifacts so future tooling can correlate commit, branch, strategy, target,
duration, and outcome.

During rollout, a non-blocking canonical-Linux full suite runs beside every
genuinely selective plan. `ci/compare-results.js` reports selected/all counts,
duration reduction, fallback, and failures found only by the full suite. A
full-only failure makes the comparison job fail visibly and requires updating
dependency edges, mappings, high-risk rules, or smoke tests before formal
adoption. Full-fallback PRs do not duplicate the same suite in the shadow job.

Keep the shadow comparison until representative changes show an acceptable
miss rate, fallback rate, flaky rate, and cost reduction. Branch protection
should require `Required selective test gate`; the shadow job remains
non-blocking during this measurement period. Removing the shadow job is a
separate, reviewed policy change. Main, release, tag, and scheduled full runs
remain after adoption.

The PR workflow caps test-process parallelism at two and uses one canonical
Linux/Node 22 binding. This limits runner startup overhead and avoids parallel
native builds fighting for CPU and memory. Cross-platform and multi-Node
coverage remains in full CI. npm downloads and Cargo registry/build outputs
are cached with separate canonical, selective, and shadow keys. To diagnose a
suspected bad cache, delete the repository cache in the CI service and rerun;
do not weaken a test or mark a failed cache-dependent command successful.

## Correcting a wrong decision

Inspect `test-plan.json` first. Then update the narrowest durable source of
truth:

- missing consumer: add a `dependsOn` edge;
- missing feature test: add a unit/integration/E2E target to its module;
- unsafe shared change: add a full-test rule and reason;
- missing project: add/register an adapter and module mappings;
- weak baseline: add an always-on smoke test;
- repeated failure relationship: promote the observed test into the module
  mapping.

Every correction needs a planner regression test. Never add an ignore rule
whose only purpose is to avoid a full fallback.
