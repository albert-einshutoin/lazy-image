# 🗺️ lazy-image Roadmap

> **Vision:** To be the most efficient, secure, and portable web image optimization engine for the cloud age.

This document reflects the current state of `develop` and focuses on the work that still meaningfully remains.

## Current State

The following foundations are already in place on `develop`:

- **Lazy, non-destructive pipeline**: deferred decode, `clone()`, file-based happy path
- **Zero-copy / mmap path**: `fromPath()` and batch-oriented file processing
- **Structured errors and operational limits**: typed error taxonomy and Image Firewall
- **Metadata-safe defaults**: strip by default, GPS strip, opt-in preservation paths
- **Benchmark and regression tooling**: benchmark docs, regression workflow, zero-copy measurement
- **Fuzzing in CI**: nightly and PR-targeted fuzz workflows with multiple targets
- **Release-size tuning**: release LTO, strip, and size-oriented profile tuning

## Current Priorities

### 1. Reliability proof for v1.x adoption

- Strengthen long-running leak and sanitizer coverage across NAPI boundaries
- Continue hardening decode / codec / metadata paths against malformed inputs
- Keep benchmark regression checks and fuzz coverage aligned with new features

### 2. Documentation and expectation management

- Keep roadmap, migration docs, benchmark docs, and compatibility docs in sync with the actual implementation
- Make workload-specific benchmark claims precise and reproducible
- Clarify partial-support areas such as metadata and disk-backed streaming

### 3. Product fit in real workloads

- Deepen the recommended file-to-file operational path
- Improve scenario-based adoption guidance for serverless, upload pipelines, and build-time optimization
- Expand observability and operational ergonomics where they help production adoption most

---

## Future Directions

### Near-term

- Broader reliability validation and memory-leak detection automation
- Clearer product docs and compatibility boundaries
- Ongoing polish for batch, byte-budget, and observability workflows

### Mid-term

- Better production ergonomics for constrained cloud/container environments
- More precise telemetry hooks and operational introspection
- Additional adoption guides and migration aids for real workloads

### Long-term

- WebAssembly / worker-oriented packaging where the architecture fits
- Native Web Streams style APIs if and when true streaming execution becomes viable

---

## 🚫 Non-Goals (What we reject)

To maintain focus and stability, the following features are explicitly **out of scope**:

1. **Drawing / Compositing**: Text rendering, watermarks, shapes.
2. **Complex Filters**: Blur, sharpen, embossing, artistic effects.
3. **Animation**: GIF/APNG creation or editing.
4. **Legacy Support**: No support for 32-bit OS or EOL Node.js versions.

---

# 🗺️ lazy-image ロードマップ

> **ビジョン:** クラウド時代における、最も効率的で、安全で、ポータブルなWeb画像最適化エンジンとなること。

このドキュメントは `develop` の現状を反映し、今後本当に残っている課題に絞って整理したものです。

## 現在の状態

`develop` にはすでに以下の基盤があります。

- **lazy / 非破壊パイプライン**: deferred decode、`clone()`、file-to-file の主導線
- **zero-copy / mmap 経路**: `fromPath()` とバッチ処理向けファイル経路
- **構造化エラーと制限**: typed error taxonomy と Image Firewall
- **安全なメタデータデフォルト**: デフォルト strip、GPS strip、必要時のみ opt-in
- **ベンチマーク運用**: benchmark docs、regression workflow、zero-copy 計測
- **CI fuzzing**: 複数ターゲットを持つ PR / nightly fuzz workflow
- **リリースサイズ最適化**: release LTO、strip、サイズ指向の profile tuning

---

## 現在の優先課題

### 1. v1.x 採用に向けた信頼性の証明

- NAPI 境界を含む長時間稼働・リーク検知の強化
- 壊れた入力に対する decode / codec / metadata 経路の継続的 hardening
- 新機能追加時も benchmark regression と fuzz coverage を維持

### 2. ドキュメントと期待値管理

- roadmap / migration / benchmark / compatibility docs を実装と同期させる
- workload ごとの benchmark claim を正確に再現可能な形で示す
- metadata や disk-backed streaming のような partial-support 領域を明確化する

### 3. 実運用でのプロダクト適合

- file-to-file の主導線をさらに明確にする
- serverless、upload pipeline、build-time optimization 向けの導入ガイドを充実させる
- 本当に運用価値の高い observability / ergonomics に投資する

---

## 将来の方向性

### 近い将来

- リーク検知や sanitizer の自動化強化
- 互換性と期待値のドキュメント整備
- batch、byte-budget、observability の運用磨き込み

### 中期

- 制約の強いクラウド / コンテナ環境向け ergonomics の向上
- テレメトリーと運用 introspection の拡張
- 実務ユースケースに沿った移行ガイドの追加

### 長期

- 適合する形での WebAssembly / worker 向け展開
- 実行モデルが整った場合の native Web Streams 風 API

---

## 🚫 やらないこと (Non-Goals)

プロジェクトの焦点と安定性を維持するため、以下の機能は明確に**スコープ外**とします。

1. **描画・合成**: テキスト描画、ウォーターマーク、図形描画など。
2. **複雑なフィルタ**: ぼかし、シャープネス、エンボス加工など。
3. **動画・アニメーション**: GIF/APNG の作成や編集。
4. **レガシーサポート**: 32bit OS や EOL を迎えた Node.js のサポート。
