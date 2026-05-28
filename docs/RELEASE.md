# Release Guide

This document describes how to cut a release of `@alberteinshutoin/lazy-image` following git flow.

Related docs:
- [SEMVER_POLICY.md](SEMVER_POLICY.md) — versioning rules and breaking change criteria
- [VERSIONING_PLAN.md](VERSIONING_PLAN.md) — current priorities and release policy
- [VERSION_HISTORY.md](VERSION_HISTORY.md) — historical version notes

---

## Overview

lazy-image follows the **git flow** branching model:

```
main         ──●─────────────────●──────────  (production, tagged releases)
                ╲               ╱
release/x.y.z   ●─●───────────●──            (release prep)
                ╲             ╲
develop      ────●─●─●─●─●─●─●──●──────────  (integration)
                  ╲ ╲ ╲ ╲ ╲ ╲
feature/...      ●─●─●─●─●─●                  (feature work)
```

Key rules (from [CLAUDE.md](../CLAUDE.md)):
- Feature branches are cut from `develop`
- Release branches are cut from `develop`
- **PRs only** — never merge directly via CLI `git merge` into `main`/`develop`
- Use GitHub UI or `gh pr merge` after review
- All CI checks must pass before merge

---

## Prerequisites

Before starting a release:

1. **`develop` is green**: all CI workflows pass on the latest commit
2. **No open release branch**: only one release in flight at a time
3. **`NPM_TOKEN` secret is valid**: check expiry in GitHub repo settings (Settings → Secrets and variables → Actions)
4. **Local tooling**: `gh`, `cargo`, `npm`, `git` installed and authenticated
5. **Decide the next version**: follow [SEMVER_POLICY.md](SEMVER_POLICY.md)
   - MAJOR (`x.0.0`): breaking changes
   - MINOR (`0.x.0`): backward-compatible additions
   - PATCH (`0.0.x`): bug fixes only

---

## Release Workflow

### Phase 1 — Prepare the release branch

```bash
# 1. Start from latest develop
git checkout develop
git pull origin develop

# 2. Create release branch (replace X.Y.Z with the target version)
git checkout -b release/X.Y.Z
```

#### Files that must be updated

All four files below need to move from the current version to the new one. Skipping any of them will fail CI or break consumers.

| File | What changes | How |
|------|--------------|-----|
| `package.json` | `version` field | Manual edit |
| `package.json` | All entries under `optionalDependencies` (6 platform packages) | Manual edit |
| `Cargo.toml` | `[package].version` | Manual edit |
| `Cargo.lock` | `lazy-image` entry | `cargo update -p lazy-image` |
| `CHANGELOG.md` | Add new `[X.Y.Z]` section under `[Unreleased]` | Manual edit |

> ⚠️ **`Cargo.lock` is required.** The Supply Chain CI job runs `cargo metadata --locked` and will fail if `Cargo.lock` does not match `Cargo.toml`. This is the most common release-time failure.

Optional helper:
```bash
npm run version   # verifies package.json ↔ Cargo.toml are in sync (does NOT bump)
```

#### CHANGELOG entry

Follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Section order:
`### Added` → `### Changed` → `### Deprecated` → `### Removed` → `### Fixed` → `### Security`

Use `git log <prev-tag>..HEAD --oneline` to enumerate merged PRs.

#### Commit and push

```bash
git add package.json Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore(release): prepare X.Y.Z"
git push -u origin release/X.Y.Z
```

---

### Phase 2 — Merge to `main` and tag

#### Open the PR

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
EOF
)"
```

#### Wait for CI

The following workflows must all succeed before merging:
- `CI` (build × 6 platforms, tests × 3 Node versions, quality, perf-safety, supply-chain, coverage, leak-detection)
- `Fuzz`
- `Security Audit`

```bash
gh pr checks <PR_NUMBER> --watch
```

#### Merge and tag

After CI is green and review is approved:

```bash
# Merge (preserves merge commit — required for git flow history)
gh pr merge <PR_NUMBER> --merge --subject "chore(release): X.Y.Z (#<PR_NUMBER>)"

# Tag the merge commit on main
git checkout main
git pull origin main
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

> ⚠️ **Do not use `git merge` directly on `main` from your local CLI.** This bypasses required PR checks and review history.

Pushing the `vX.Y.Z` tag triggers the `Publish to npm` job in [CI.yml](../.github/workflows/CI.yml). The job:
1. Downloads the prebuilt `.node` artifacts for all 6 platforms
2. Publishes each platform package (`@alberteinshutoin/lazy-image-{platform}`)
3. Publishes the main package
4. Creates a GitHub Release with auto-generated notes

---

### Phase 3 — Sync back to `develop`

The release branch contains version bumps and CHANGELOG entries that must flow back to `develop`. Skipping this leaves `develop` perpetually behind.

```bash
gh pr create \
  --base develop \
  --head release/X.Y.Z \
  --title "chore(develop): sync X.Y.Z from release" \
  --body "Sync version bump and CHANGELOG from release/X.Y.Z back to develop."
```

After CI passes:
```bash
gh pr merge <SYNC_PR_NUMBER> --merge --subject "chore(develop): sync X.Y.Z from release (#<SYNC_PR_NUMBER>)"
```

---

### Phase 4 — Cleanup

```bash
git checkout develop
git pull origin develop
git branch -d release/X.Y.Z
git push origin --delete release/X.Y.Z
```

---

## Post-Release Verification

Confirm the release landed everywhere:

```bash
# 1. npm main package
npm view @alberteinshutoin/lazy-image version
# → should print X.Y.Z

# 2. All platform packages (must match main version)
for pkg in darwin-arm64 darwin-x64 linux-x64-gnu linux-x64-musl linux-arm64-gnu win32-x64-msvc; do
  echo "$pkg: $(npm view @alberteinshutoin/lazy-image-$pkg version)"
done

# 3. GitHub Release exists
gh release view vX.Y.Z --json name,tagName,isDraft,isPrerelease,url
```

Smoke test the published package:
```bash
mkdir /tmp/lazy-image-smoke && cd /tmp/lazy-image-smoke
npm init -y
npm install @alberteinshutoin/lazy-image@X.Y.Z
node -e "const li = require('@alberteinshutoin/lazy-image'); console.log(Object.keys(li))"
```

---

## Common Pitfalls

Issues observed in past releases. Check these first when something breaks.

### 1. `Cargo.lock` not updated → Supply Chain fails

**Symptom:** `Supply Chain` job fails with:
```
error: the lock file /home/runner/.../Cargo.lock needs to be updated but --locked was passed
```

**Fix:** After bumping `Cargo.toml`, run `cargo update -p lazy-image` locally and commit the resulting `Cargo.lock`.

**Prevention:** Always stage `Cargo.lock` together with `Cargo.toml` in the release prep commit.

### 2. `NPM_TOKEN` expired → publish job fails with 404

**Symptom:** `Publish to npm` job fails with:
```
npm error 404 Not Found - PUT https://registry.npmjs.org/@alberteinshutoin%2flazy-image-...
```

The 404 is misleading — it's actually an authentication failure for existing packages.

**Fix:**
1. Generate a new automation token at [npmjs.com/settings/.../tokens](https://www.npmjs.com/settings/~/tokens)
2. Update the `NPM_TOKEN` secret in GitHub (Settings → Secrets and variables → Actions)
3. Re-run the failed `Publish to npm` job: `gh run rerun <RUN_ID> --failed`

**Prevention:** Set a calendar reminder before token expiry.

### 3. `optionalDependencies` versions not bumped → main package install fails

**Symptom:** Users installing the new version see missing optional dependency warnings or runtime errors loading the native binding.

**Fix:** Update all 6 entries under `optionalDependencies` in `package.json` to match the new version.

**Prevention:** Search for the old version string before committing:
```bash
grep -n "0\.X\.Y" package.json
```

### 4. Direct CLI merge into `main` → bypasses CI requirements

**Symptom:** A `git merge` into `main` from a local checkout pushes without going through PR review.

**Fix:** Revert with a PR, then re-do via `gh pr merge`.

**Prevention:** Enable branch protection on `main` requiring PR review and status checks. Never run `git push origin main` after a local merge.

### 5. Skipping the sync-back PR → `develop` drifts from `main`

**Symptom:** Subsequent releases re-introduce the previous version because `develop` was never updated.

**Fix:** Always complete Phase 3 (sync back PR) immediately after Phase 2.

**Prevention:** Treat the release as incomplete until Phase 4 cleanup has run.

---

## Hotfix Procedure

For urgent fixes that cannot wait for the next regular release:

```bash
# Cut hotfix from main (not develop)
git checkout main
git pull origin main
git checkout -b hotfix/X.Y.(Z+1)

# Fix, test, bump PATCH version (apply Phase 1 file changes)
# Open PRs to BOTH main and develop
gh pr create --base main --head hotfix/X.Y.Z+1 --title "fix: <issue>"
gh pr create --base develop --head hotfix/X.Y.Z+1 --title "fix: <issue> (sync)"
```

After both PRs merge, tag and push as in Phase 2. Delete the hotfix branch.

---

## Files Touched in Every Release

For quick reference, the canonical set of files updated for a routine version bump:

```
package.json        # version + optionalDependencies (×6)
Cargo.toml          # [package].version
Cargo.lock          # lazy-image entry (via `cargo update -p lazy-image`)
CHANGELOG.md        # new [X.Y.Z] section
```

CI workflows ([`.github/workflows/CI.yml`](../.github/workflows/CI.yml)) handle the rest:
- Multi-platform binary builds
- npm publishing
- GitHub Release creation
- Provenance attestation
