# Semantic Versioning & Deprecation Policy

This document defines how **lazy-image** uses Semantic Versioning (SemVer) and how we announce and remove breaking changes.

## Scope of Stability
- JavaScript/TypeScript API (`@alberteinshutoin/lazy-image`, including `ImageEngine`, `createStreamingPipeline`, and exported types)
- N-API binary interface (prebuilt platform packages)
- Documented behaviors in `README.md`, `README.ja.md`, and `spec/` (resize rules, metadata defaults, limits, error taxonomy)
- Supported Node.js versions (currently `>=18`)

Rust crate internals are not a public surface; they may change without notice as long as the JS API remains stable.

## Versioning Rules
We follow SemVer 2.0.0.

- **MAJOR (x.0.0)**: Introduces breaking changes or removes deprecated features. Examples:
  - Removing options or methods, changing parameter order/types
  - Changing default behaviors that affect outputs (e.g., default quality, metadata stripping policy)
  - Dropping support for a Node.js version
  - Removing a previously supported input/output format
  - Renaming/removing error codes or changing thrown error categories
- **MINOR (x.y.0)**: Backward-compatible additions and improvements.
  - New methods/options with safe defaults
  - Performance improvements that do not alter observable behavior
  - New error codes for previously undefined invalid inputs (without changing success paths)
- **PATCH (x.y.z)**: Bug fixes and non-behavioral changes.
  - Fixes for incorrect behavior that bring outputs in line with documented intent
  - Dependency updates without API surface change
  - Documentation and tooling updates

### Pre-1.0 policy
Even while on 0.x, we treat MINOR as additive and avoid breaking changes without the deprecation process below. True breaking changes should still ship in the next MAJOR bump (e.g., 0.x → 1.0.0).

## What Counts as a Breaking Change
- Requires code changes for existing consumers, or materially changes outputs/metadata/error categories under the same inputs
- Changes default quality/preset values, fit/resize semantics, metadata defaults, or firewall limits
- Removes or makes previously accepted inputs invalid (formats, options, Node.js versions)
- Changes TypeScript public types in a way that fails existing valid code

Clarifications that align implementation with documented behavior are **not** treated as breaking when they fix a bug.

## Deprecation Process
1. **Mark and document**: Add a `Deprecated` note to the relevant docs (README/spec/API tables) and to `CHANGELOG.md` under **Deprecated**.
2. **Announce**: Include deprecation in the release notes for the MINOR version where it first appears.
3. **Provide alternatives**: Document the replacement API or configuration.
4. **Grace period**: Removal happens in the **next MAJOR**. If a MAJOR is not imminent, keep the deprecated path for **at least one full MINOR cycle** after the initial deprecation release.
5. **Runtime signals (when feasible)**: Emit non-fatal warnings (e.g., `console.warn`) for deprecated options where this does not cause noisy logs in typical server usage.

## Changelog Standard
We use [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/) with the following sections under **Unreleased** and each release:
- Added
- Changed
- Deprecated
- Removed
- Fixed
- Performance
- Security

Each PR that changes user-visible behavior must update `CHANGELOG.md` accordingly (or confirm “no user-facing change”). Deprecations must be listed under **Deprecated**, and removals under **Removed** with the version that performed the removal.

## Contribution Checklist (versioning)
- Does this change require a MAJOR bump? If yes, add to **Unreleased → Removed** and plan the next MAJOR.
- If breaking but postponed: mark as **Deprecated**, document alternative, and keep behavior until the next MAJOR.
- If additive: ensure defaults are safe; version bump should be MINOR.
- If a bug fix: PATCH is sufficient; note under **Fixed**.

## Communication Channels
- `CHANGELOG.md` (single source of truth)
- Release notes on GitHub
- README/spec updates when behavior changes
- For high-impact removals, add a short note in `docs/` alongside the relevant feature (e.g., `docs/OPERATIONS.md`)
