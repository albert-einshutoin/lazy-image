# 📋 Versioning Plan & Issue Prioritization

> **Current Version**: v0.9.0 (2026-01-21)  
> **Goal**: v1.0.0 "Production Ready" release

このドキュメントは、v0.9.0からv1.0.0までのバージョニングプランと、各issueの優先順位を定義します。

---

## 🎯 Version Roadmap Overview

```
v0.9.0 (Current) ──┐
                    │
                    └──> v1.0.0 ──> v1.x ──> v2.0
                    "Production Ready"  "Serverless"  "Universal"
```

---

## 📦 v0.9.0 - "The Optimizer" (Next Release)

**目標**: パフォーマンスと効率性の最適化、v1.0.0への準備

### Must-Have (v0.9.0必須)

#### P0 - Critical
- **#83**: stability: Add memory leak detection to CI with Valgrind/Sanitizers
  - **理由**: v1.0.0の前提条件。CIでの自動検知が必須
  - **状態**: 部分的に実装済み（sanitizersジョブは存在）が、完全な統合が必要
  - **作業**: CIジョブの完全な統合とドキュメント化

#### P1 - High Priority
- **#132**: performance: Replace fast_image_resize Fallback Logic (確実性の担保)
  - **理由**: パフォーマンスの予測可能性はv1.0.0の品質要件
  - **影響**: すべての画像で一貫した高速処理を保証
  - **作業**: フォールバックロジックの除去、データ正規化の実装

- **#133**: security: Safety Audit & Abstraction for libavif FFI (AVIFの安全化)
  - **理由**: メモリ安全性はv1.0.0の必須要件
  - **影響**: AVIFエンコーディングの安全性向上
  - **作業**: SafeAvifImageラッパーの実装

- **#106**: 虚偽の「ColorSpace」APIの撤廃または実装
  - **理由**: APIの一貫性と明確性。v1.0.0前に解決すべき
  - **影響**: ユーザー混乱の解消、APIの明確化
  - **作業**: 実装または完全な撤廃

### Should-Have (v0.9.0推奨)

#### P2 - Medium Priority
- **#129**: refactor: Decompose ImageEngine (神クラスの解体)
  - **理由**: 保守性向上。v1.0.0前のリファクタリング推奨
  - **影響**: コードレビュー効率向上、バグ発見率向上
  - **作業**: モジュール分割の実装

- **#130**: performance: Minimize NAPI Copy Overhead (コピーゼロ化)
  - **理由**: パフォーマンス改善。大きな画像処理で効果的
  - **影響**: メモリコピーオーバーヘッドの削減
  - **作業**: External Bufferの実装

- **#100**: test: カバレッジ改善のためのテスト追加
  - **理由**: テストカバレッジの向上はv1.0.0の品質要件
  - **影響**: バグ発見率の向上
  - **作業**: 不足しているテストケースの追加

- **#111**: ADR実装と現状の乖離の解消
  - **理由**: ドキュメントの正確性はv1.0.0の品質要件
  - **影響**: 開発者体験の向上
  - **作業**: ADRと実装の整合性確認と更新

### Nice-to-Have (v0.9.0オプション)

#### P3 - Low Priority
- **#131**: performance: Optimize Error Type Conversions (エラーのアロケーション排除)
  - **理由**: マイクロ最適化。影響は限定的
  - **影響**: エラー発生時のパフォーマンス向上（限定的）
  - **作業**: `Cow<'static, str>`への移行

---

## 🏆 v1.0.0 - "Production Ready" (Stable Release)

**目標**: 企業採用に耐えうる「信頼性」の証明

### Must-Have (v1.0.0必須)

#### P0 - Critical
- **#83**: stability: Add memory leak detection to CI with Valgrind/Sanitizers
  - **状態**: v0.9.0で実装済みである必要がある
  - **検証**: CIで全てのメモリリーク検知テストがパスすること

#### P1 - High Priority
- **#128**: performance: True Zero-Copy / Memory Mapping Implementation (メモリマップの導入)
  - **理由**: 大容量ファイル処理でのOOMリスク削減はv1.0.0の品質要件
  - **影響**: 4GB以上のファイルでも安全に処理可能
  - **作業**: memmap2クレートの統合

- **#132**: performance: Replace fast_image_resize Fallback Logic
  - **状態**: v0.9.0で実装済みである必要がある

- **#133**: security: Safety Audit & Abstraction for libavif FFI
  - **状態**: v0.9.0で実装済みである必要がある

- **#106**: 虚偽の「ColorSpace」APIの撤廃または実装
  - **状態**: v0.9.0で実装済みである必要がある

### Should-Have (v1.0.0推奨)

#### P2 - Medium Priority
- **#129**: refactor: Decompose ImageEngine
  - **状態**: v0.9.0で実装済みであることが推奨される

- **#130**: performance: Minimize NAPI Copy Overhead
  - **状態**: v0.9.0で実装済みであることが推奨される

- **#100**: test: カバレッジ改善のためのテスト追加
  - **状態**: v0.9.0で実装済みであることが推奨される

### 既に実装済み（確認済み）

- ✅ **#82**: security: Implement fuzzing tests with cargo-fuzz (CLOSED)
  - ファジングテストは実装済み（FUZZING.md参照）
  - CIへの統合は検討が必要

- ✅ **#84**: api: Final API freeze and TypeScript definitions audit (CLOSED)
  - API凍結は完了

---

## 🚀 v1.x - "Serverless Native" (Future)

**目標**: クラウドネイティブ機能の強化

### P1 - High Priority
- **#85**: performance: Smart concurrency with auto memory cap detection
  - **理由**: サーバーレス環境でのOOM回避
  - **影響**: 512MBコンテナでの安全な動作
  - **作業**: メモリ検出と自動調整の実装

### P3 - Low Priority
- **#86**: observability: Add telemetry hooks for detailed metrics
  - **理由**: 運用監視のためのメトリクス
  - **影響**: パフォーマンス分析の容易化

---

## 🌐 v2.0 - "Universal Engine" (Long Term)

**目標**: Node.jsの枠を超える

### P3 - Low Priority
- **#87**: wasm: Add WebAssembly support for Cloudflare Workers and browsers
- **#88**: api: Add native support for Web Streams API

---

## 📊 Issue Priority Summary

### v0.9.0 Must-Have
1. **#83** (P0) - メモリリーク検知CI統合
2. **#132** (P1) - fast_image_resizeフォールバック除去
3. **#133** (P1) - AVIF FFI安全化
4. **#106** (P1) - ColorSpace API整理

### v0.9.0 Should-Have
5. **#129** (P2) - ImageEngine分解
6. **#130** (P2) - NAPIコピーゼロ化
7. **#100** (P2) - テストカバレッジ改善
8. **#111** (P2) - ADR整合性

### v1.0.0 Must-Have
9. **#128** (P1) - メモリマップ実装

### v1.x Future
10. **#85** (P1) - スマートコンカレンシー
11. **#86** (P3) - テレメトリー
12. **#87** (P3) - WASMサポート
13. **#88** (P3) - Web Streams API

---

## 🗓️ Release Timeline (推奨)

### v0.9.0 - "The Optimizer"
**目標リリース**: 2026年2月
**マイルストーン**:
- Week 1-2: P0/P1必須タスクの実装 (#83, #132, #133, #106)
- Week 3-4: P2推奨タスクの実装 (#129, #130, #100, #111)
- Week 5: テスト、ドキュメント更新、リリース準備

### v1.0.0 - "Production Ready"
**目標リリース**: 2026年3-4月
**マイルストーン**:
- Week 1-2: #128 (メモリマップ実装)
- Week 3: 全機能の統合テスト、パフォーマンス検証
- Week 4: ドキュメント最終確認、リリース準備

### v1.x - "Serverless Native"
**目標リリース**: 2026年後半
- #85 (スマートコンカレンシー)
- #86 (テレメトリー)

### v2.0 - "Universal Engine"
**目標リリース**: 2027年以降
- #87 (WASM)
- #88 (Web Streams)

---

## ✅ v1.0.0 Release Checklist

### 必須要件
- [ ] メモリリーク検知がCIで完全に動作している (#83)
- [ ] fast_image_resizeフォールバックが除去されている (#132)
- [ ] AVIF FFIが安全化されている (#133)
- [ ] ColorSpace APIが整理されている (#106)
- [ ] メモリマップ実装が完了している (#128)
- [ ] ファジングテストがCIで実行されている（#82は実装済み、CI統合を確認）
- [ ] API凍結が完了している（#84はCLOSED、最終確認）

### 推奨要件
- [ ] ImageEngineがモジュール分割されている (#129)
- [ ] NAPIコピーゼロ化が実装されている (#130)
- [ ] テストカバレッジが十分である (#100)
- [ ] ADRと実装が整合している (#111)

### 品質要件
- [ ] 全テストがパスしている
- [ ] ベンチマークが安定している
- [ ] ドキュメントが最新である
- [ ] セキュリティ監査が完了している

---

## 📝 Notes

- **v0.9.0**はv1.0.0への準備リリース。必須タスクを優先し、推奨タスクは時間があれば実装
- **v1.0.0**は安定版リリース。破壊的変更は行わない
- **v1.x**は機能追加リリース。後方互換性を維持
- **v2.0**は破壊的変更を含む可能性がある
