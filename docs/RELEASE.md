# Release Guide

This document describes how to cut a release of `@alberteinshutoin/lazy-image` with GitHub Flow.

Related docs:
- [SEMVER_POLICY.md](SEMVER_POLICY.md) — versioning rules and breaking change criteria
- [VERSIONING_PLAN.md](VERSIONING_PLAN.md) — current priorities and release policy
- [VERSION_HISTORY.md](VERSION_HISTORY.md) — historical version notes

---

## Overview

lazy-image uses **GitHub Flow**:

```text
main            ──●──────●──────●──────●──────  (protected default branch, tagged releases)
                    ╲      ╲      ╲
feature/fix/...      ●──●   ●──●   ●──●          (short-lived PR branches)
release/x.y.z                ●──●                (short-lived release prep branch)
```

Key rules:
- `main` is the only long-lived branch.
- Feature, fix, docs, chore, hotfix, and release-prep branches are cut from latest `main`.
- All changes land through PRs back to `main`.
- Do not merge directly from a local CLI into `main`; use GitHub UI or `gh pr merge` after review.
- All required CI checks must pass before merge.
- Release tags are created from the merged `main` commit.

---

## Prerequisites

Before starting a release:

1. **`main` is green**: all required CI workflows pass on the latest commit.
2. **No open release PR**: only one release in flight at a time.
3. **`NPM_TOKEN` secret is valid**: check expiry in GitHub repo settings (Settings -> Secrets and variables -> Actions).
4. **Local tooling**: `gh`, `cargo`, `npm`, `git` installed and authenticated.
5. **Decide the next version**: follow [SEMVER_POLICY.md](SEMVER_POLICY.md).
   - MAJOR (`x.0.0`): breaking changes
   - MINOR (`0.x.0`): backward-compatible additions
   - PATCH (`0.0.x`): bug fixes only
6. **Check breaking-change markers**: if the target changelog section contains
   `BREAKING` or `Removed`, the release version must be a major boundary
   (`x.0.0`). For current 0.x work, that means `1.0.0`.

---

## Release Workflow

### Phase 1 — Prepare the release branch

```bash
# 1. Start from latest main
git checkout main
git pull --ff-only origin main

# 2. Create a short-lived release branch
git checkout -b release/X.Y.Z
```

#### Files that must be updated

The files below need to move from the current version to the new one. Skipping any of them will fail CI or break consumers.

| File | What changes | How |
|------|--------------|-----|
| `package.json` | `version` field | Manual edit |
| `package.json` | All entries under `optionalDependencies` (6 platform packages) | Manual edit |
| `packages/lazy-image-wasm/package.json` | `version` field | Manual edit |
| `packages/lazy-image-wasm/shared.js` | Exported `VERSION` constant | Manual edit |
| `package-lock.json` | Root `version`, root `packages[""]` block, workspace package entry, and the six `@alberteinshutoin/lazy-image-*` optional dependency entries for the new version. Before publish, npm may keep the platform package lock entries as optional placeholders because the new packages are not in the registry yet. | `npm install --package-lock-only --ignore-scripts` |
| `Cargo.toml` | `[package].version` | Manual edit |
| `Cargo.lock` | `lazy-image` entry | `cargo update -p lazy-image` |
| `index.js` | Generated native loader version checks | `npm run build` |
| `CHANGELOG.md` | Add new `[X.Y.Z]` section under `[Unreleased]` | Manual edit |

> **`Cargo.lock` is required.** The Supply Chain CI job runs `cargo metadata --locked` and will fail if `Cargo.lock` does not match `Cargo.toml`.
>
> **`package-lock.json` is required.** The release PR can pass before the new platform packages exist on npm, but later `npm ci` runs will fail once those packages are published if the lock file still points at the previous version.

Optional helper:

```bash
npm run version
npm run release:check
```

#### CHANGELOG entry

Follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Section order:
`### Added` -> `### Changed` -> `### Deprecated` -> `### Removed` -> `### Fixed` -> `### Security`

Use `git log <prev-tag>..HEAD --oneline` to enumerate merged PRs.

If a release section contains `### Changed (BREAKING)`, `BREAKING` bullets, or
`### Removed`, run `npm run release:check` before committing the release branch.
The command uses the version in `package.json` by default; it also accepts an
explicit version for planning checks:

```bash
node scripts/check-release-policy.js 1.0.0
node scripts/check-release-policy.js 0.16.0
```

#### Commit and push

```bash
git add package.json packages/lazy-image-wasm/package.json packages/lazy-image-wasm/shared.js package-lock.json Cargo.toml Cargo.lock index.js CHANGELOG.md
git commit -m "chore(release): prepare X.Y.Z"
git push -u origin release/X.Y.Z
```

---

### Phase 2 — Open the PR to `main`

```bash
gh pr create \
  --base main \
  --head release/X.Y.Z \
  --title "chore(release): X.Y.Z" \
  --body "$(cat <<'EOF'
## Release X.Y.Z

### Summary
<1-3 bullets>

### Highlights
- ...

### Test Checklist
- [ ] All CI jobs green
- [ ] Quality gate passes
- [ ] Perf-safety gate passes
- [ ] `npm run release:check` passes
EOF
)"
```

Required checks before merging:
- `CI` (multi-platform build, tests, quality, perf-safety, supply-chain, coverage, leak detection)
- `Fuzz`
- `Security Audit`

```bash
gh pr checks <PR_NUMBER> --watch
```

---

### Phase 3 — Merge, tag, and publish

After CI is green and review is approved:

```bash
# Merge through GitHub so branch protection and review history are preserved.
gh pr merge <PR_NUMBER> --merge --subject "chore(release): X.Y.Z (#<PR_NUMBER>)"

# Tag the merged main commit.
git checkout main
git pull --ff-only origin main
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

Pushing the `vX.Y.Z` tag triggers the publish job in [CI.yml](../.github/workflows/CI.yml). The job:
1. Downloads the prebuilt `.node` artifacts for all supported platforms.
2. Publishes each platform package (`@alberteinshutoin/lazy-image-{platform}`).
3. Publishes the Wasm workspace package (`@alberteinshutoin/lazy-image-wasm`).
4. Publishes the main package.
5. Creates a GitHub Release with generated notes.

---

### Phase 4 — Cleanup

```bash
git branch -d release/X.Y.Z
git push origin --delete release/X.Y.Z
```

---

## Post-Release Verification

Confirm the release landed everywhere:

```bash
# 1. npm main package
npm view @alberteinshutoin/lazy-image version

# 2. All platform packages must match the main version
for pkg in darwin-arm64 darwin-x64 linux-x64-gnu linux-x64-musl linux-arm64-gnu win32-x64-msvc; do
  echo "$pkg: $(npm view @alberteinshutoin/lazy-image-$pkg version)"
done

# 3. Wasm package
npm view @alberteinshutoin/lazy-image-wasm version

# 4. GitHub Release exists
gh release view vX.Y.Z --json name,tagName,isDraft,isPrerelease,url
```

Smoke test the published packages:

```bash
mkdir /tmp/lazy-image-smoke && cd /tmp/lazy-image-smoke
npm init -y
npm install @alberteinshutoin/lazy-image@X.Y.Z
node -e "const li = require('@alberteinshutoin/lazy-image'); console.log(Object.keys(li))"

npm install @alberteinshutoin/lazy-image-wasm@X.Y.Z
node --input-type=module -e "import { VERSION } from '@alberteinshutoin/lazy-image-wasm/shared'; console.log(VERSION)"
```

---

## Common Pitfalls

### 1. `Cargo.lock` not updated -> Supply Chain fails

**Symptom:** `Supply Chain` job fails with:

```text
error: the lock file /home/runner/.../Cargo.lock needs to be updated but --locked was passed
```

**Fix:** After bumping `Cargo.toml`, run `cargo update -p lazy-image` locally and commit the resulting `Cargo.lock`.

**Prevention:** Always stage `Cargo.lock` together with `Cargo.toml` in the release prep commit.

### 2. `NPM_TOKEN` expired -> publish job fails with 404

**Symptom:** `Publish to npm` job fails with:

```text
npm error 404 Not Found - PUT https://registry.npmjs.org/@alberteinshutoin%2flazy-image-...
```

The 404 is misleading; it is usually an authentication failure for existing packages.

**Fix:**
1. Generate a new automation token at [npmjs.com/settings/.../tokens](https://www.npmjs.com/settings/~/tokens).
2. Update the `NPM_TOKEN` secret in GitHub (Settings -> Secrets and variables -> Actions).
3. Re-run the failed publish job: `gh run rerun <RUN_ID> --failed`.

**Prevention:** Set a calendar reminder before token expiry.

### 3. `optionalDependencies` versions not bumped -> install fails

**Symptom:** Users installing the new version see missing optional dependency warnings or runtime errors loading the native binding.

**Fix:** Update all 6 entries under `optionalDependencies` in `package.json` to match the new version.

**Prevention:** Search for the old version string before committing:

```bash
grep -n "0\\.X\\.Y" package.json
```

### 4. Direct CLI merge into `main` -> bypasses CI requirements

**Symptom:** A local merge is pushed to `main` without PR review and required status checks.

**Fix:** Revert with a PR, then re-do the change via PR.

**Prevention:** Keep branch protection enabled on `main`. Never run `git push origin main` after a local merge.

### 5. `package-lock.json` not bumped -> post-release PRs break

**Symptom:** PRs opened after publish start failing in `npm ci` with lock-file mismatch errors for `@alberteinshutoin/lazy-image-*`.

**Why the release PR may miss it:** before the tag publishes the new platform packages, npm can treat missing optional dependencies as skipped. After publish, npm sees the new package versions and enforces the lock file strictly.

**Fix:** Open a follow-up PR to `main`:

```bash
git checkout main
git pull --ff-only origin main
git checkout -b chore/sync-package-lock-X.Y.Z
npm install --package-lock-only --ignore-scripts
git add package-lock.json
git commit -m "chore: sync package-lock.json to X.Y.Z"
git push -u origin chore/sync-package-lock-X.Y.Z
gh pr create --base main --title "chore: sync package-lock.json to X.Y.Z"
```

**Prevention:** Update `package-lock.json` in Phase 1 with the rest of the version bump files.

---

## Hotfix Procedure

For urgent fixes that cannot wait for the next regular release:

```bash
git checkout main
git pull --ff-only origin main
git checkout -b hotfix/X.Y.(Z+1)

# Fix, test, bump PATCH version if needed, then open a PR to main.
gh pr create --base main --head hotfix/X.Y.Z+1 --title "fix: <issue>"
```

After the PR merges, tag and push as in Phase 3. Delete the hotfix branch.

---

## Files Touched in Every Release

For quick reference, the canonical set of files updated for a routine version bump:

```text
package.json        # version + optionalDependencies (x6)
packages/lazy-image-wasm/package.json  # workspace package version
packages/lazy-image-wasm/shared.js     # exported VERSION constant
package-lock.json   # root version + workspace + 6 lazy-image-* optional entries
Cargo.toml          # [package].version
Cargo.lock          # lazy-image entry
index.js            # generated native loader version checks
CHANGELOG.md        # new [X.Y.Z] section
```

CI workflows ([`.github/workflows/CI.yml`](../.github/workflows/CI.yml)) handle the rest:
- Multi-platform binary builds
- npm publishing
- GitHub Release creation
- Provenance attestation
