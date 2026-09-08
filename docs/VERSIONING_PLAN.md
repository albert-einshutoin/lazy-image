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

v1.3.0 is a backward-compatible release focused on closing the remaining
artifact-compiler evidence gap. The following compiler hardening is already on
`main` and is part of the release baseline:

- #827: transparent AVIF artifact verification
- #828: bounded-buffer JPEG container verification
- #829: distinction between absent and unparseable EXIF Orientation
- #830: benchmark claims are already aligned with the canonical historical
  numbers and measurement context; the tracker is reconciled as complete

The remaining release gate is:

- #769: record real-image, perceptual-quality, budget, memory, and E2E compiler
  evidence, including the #827/#828/#829 regression cases

#696 (the thin `compileImage()` CLI) stays outside v1.3.0 until #769 is
complete. It adds a public filesystem entrypoint and needs its own API,
packaging, and security review; bundling it now would make the release scope
larger without closing the evidence gate.

v1.3.0 is release-ready only after #769 is resolved, the resulting claims are
synchronized, and the published native/Wasm package smoke evidence is
recorded. Merged implementation and published-package evidence remain
separate release claims.

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
- #703（artifact compiler）はv1.3.0で#827/#828/#829/#830の完了を含め、#769を
  release gateとして追跡する。#696（CLI）は#769後の後続リリースで扱う。
- #645（Wasm runtime expansion）と#88（Web Streams）はv2.0以降の計画であり、
  v1.3.0にも含めない。

詳細な中長期方針は [ROADMAP.md](./ROADMAP.md) を参照してください。
