# 📋 Versioning Plan & Priorities

> **Current package version**: v1.0.x
> **Working baseline**: `main`

このドキュメントは、古い issue 番号ベースの固定計画ではなく、`main` の現状に合った優先順位を簡潔に示します。

## 現在の優先順位

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
- #703（artifact compiler）、#769（実画像 benchmark evidence）、#645
  （Wasm runtime expansion）は additive な将来作業であり、v1.0.1 の blocker
  にはしない。
- v1.0.1 は公開契約・ドキュメント・release governance の整合修正に限定し、
  API removal や新しい runtime assumption を含めない。

詳細な中長期方針は [ROADMAP.md](./ROADMAP.md) を参照してください。
