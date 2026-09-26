# lazy-image verification commands

Use the existing package scripts and GitHub workflows; do not duplicate their implementations.

| Purpose | Canonical command or workflow |
|---|---|
| Native build | `npm run build` |
| Complete local tests | `npm test` |
| Existing benchmark and claim checks | `npm run test:bench` |
| Compiler/quality release evidence | `npm run test:bench:release` |
| PR impact plan and execution | `npm run ci:plan-tests`, `npm run ci:run-tests` |
| Complete hosted validation | `.github/workflows/CI.yml`, `full_validation: true` on manual runs |
| Scheduled benchmark evidence and artifact retention | `.github/workflows/benchmark-regression.yml` |
| Security | `.github/workflows/security.yml` |
| Release candidate native smoke | `.github/workflows/CI.yml` (tag or manual dry run): `stage-release-artifacts.cjs` checks six binary artifacts and packs tarballs, then `registry-native-smoke.cjs` verifies all six platforms on Node 22/24 before publish |
| Published npm native smoke | Manually run `.github/workflows/registry-native-smoke.yml` with the exact published `version`; `node scripts/registry-native-smoke.cjs` requires `SMOKE_VERSION`, `SMOKE_EXPECTED_PLATFORM`, and optional `SMOKE_REPORT_PATH` |

Review changes and run targeted checks before complete validation. Release evidence must retain failed/unmet outcomes, source/license hashes, and exact measurement scope. Expected hostile-input rejection is separate from valid-input acceptance. Unknown or unsafe impact analysis requires full validation; scheduled/main/release validation must not be reduced to a PR subset.
