# 📋 Versioning Plan & Priorities

> **Current package version**: v1.2.0
> **Working baseline**: `main`

このドキュメントは、古い issue 番号ベースの固定計画ではなく、`main` の現状に合った優先順位を簡潔に示します。

## 現在の優先順位

### v1.2.0 release scope

- #697: `toResponsiveSet()` / `toFilesResponsive()` responsive output helpers
- #698: `toPlaceholder()` LQIP / `blurDataURL` helper
- API types, documentation, examples, and generated artifacts
- Native, JavaScript, Rust, Wasm package, pack, and release-policy verification

The responsive and placeholder helpers are additive Node.js APIs. They reuse
the existing lazy pipeline and do not change the native ABI or the
privacy-safe `compileImage()` contract.

### v1.3.0 planned scope (#703)

v1.3.0 is a backward-compatible release of the hardened artifact compiler and
its thin CLI entrypoint. As of 2026-09-10, the following work is merged:

- #827 / PR #831: transparent AVIF artifact verification
- #828 / PR #832: bounded-buffer JPEG container verification
- #829 / PR #836: distinction between absent and unparseable EXIF Orientation
- #838 / PR #840: decodable transparent WebP output with ICC
- #830 / PR #839: performance explanations aligned with historical benchmarks
- #696 / PR #843: `lazy-image compile` CLI, documentation, cancellation tests,
  and installed-tarball smoke coverage
- #769 / PR #841 and #846: a three-case compiler corpus runner and portable
  license/checksum validation; this is partial evidence, not completion of #769

#### Final candidate verification

The preparation PR expands the corpus to 15 cases and adds isolated repeated
compiler/quality measurements plus scheduled artifact retention
(`npm run test:bench:release`). Final hosted evidence is required before marking
#769 complete. See [BENCHMARK_OPERATIONS.md](./BENCHMARK_OPERATIONS.md).

1. Verify #769 on the exact candidate with `npm run test:bench:release` and
   retain its hosted JSON/Markdown evidence. All supported inputs and strict
   compiler budgets must pass; expected hostile rejections are separate.
2. The preparation PR synchronizes Node/Wasm/Rust versions, lockfiles, generated
   loader and changelog to 1.3.0. Require build, full tests, security checks and
   `release:check` for that exact candidate.
3. Verify packed release artifacts and the CLI with locally supplied candidate
   platform packages. The existing CLI smoke uses
   `NAPI_RS_NATIVE_LIBRARY_PATH`; it does not prove registry platform-package
   downloads. Record the tested platforms and any remaining coverage gaps.

#### Publication conditions

The npm publishing token was renewed on 2026-09-11 JST (GitHub secret update
verified). Publication is authorized after the remaining release gates pass.
The planned release date is 2026-09-11; preparing that changelog entry does not
mean npm publication has completed.

Before authorized publication, verify package
ownership/credential readiness, follow [RELEASE.md](./RELEASE.md), and then
verify published native/platform, Wasm, and CLI installation without local
native-library overrides. Post-publication smoke is separate from the
pre-publication release gate.

### P1

- 長時間稼働と NAPI 境界を含むリーク検知の強化
- benchmark / migration / compatibility / roadmap の整合性維持
- metadata と streaming の部分対応領域の期待値管理

### P2

- serverless / upload / build pipeline 向けの導入ドキュメント拡充
- observability と batch ergonomics の改善
- 既存 benchmark / fuzz / regression 運用の継続的改善

### P3

- worker / wasm 向け展開の検討
- true streaming execution model が成立する場合の新 API 検討

## リリース方針

- patch/minor: additive changes, docs alignment, performance/safety improvements
- major: API removal, semantic shifts, or new execution-model assumptions
- `1.0.0` は Wasm error contract と Node.js runtime floor の breaking change を
  major boundary として公開する。
- release branch では version/changelog 更新後に `npm run release:check` を実行し、
  breaking marker を含む non-major release を拒否する。

## v1.x 互換性契約

- `toBufferWithMetrics()` などの既存 output convenience API は v1.x では
  維持し、削除は v2.0.0 でのみ検討する。v1 より前に削除済みの metrics
  aliases は v1.x の公開契約に含めない。
- #703（artifact compiler）はv1.3.0で#827/#828/#829の完了を含め、#830と#696の完了を追跡する。#769の比較証跡と公開前検証は引き続き
  release gateとし、候補の最終検証後に公開可否を判断する。
- #645（Wasm runtime expansion）と#88（Web Streams）はv2.0以降の計画であり、
  v1.3.0にも含めない。

詳細な中長期方針は [ROADMAP.md](./ROADMAP.md) を参照してください。
