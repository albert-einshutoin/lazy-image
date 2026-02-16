Here is the updated `ROADMAP.md` organized by OSS standards.
It strictly separates "Completed" from "Planned" tasks, details specific goals for upcoming versions, and is divided into English (top) and Japanese (bottom) sections.

---

# 🗺️ lazy-image Roadmap

> **Vision:** To be the most efficient, secure, and portable web image optimization engine for the cloud age.

This document outlines the development status and future direction of `lazy-image`. We follow Semantic Versioning.

## ✅ Completion Status Checklist

### 🏗️ Architecture & Core (v0.8.x - Completed)

* [x] **True Lazy Loading**: Defer file I/O until necessary (`fromPath`).
* [x] **Zero-Copy Architecture**: Prevent pixel copying during format conversion.
* [x] **Thread Safety**: Fix libuv/rayon thread pool conflicts.
* [x] **Structured Error Handling**: Replace string errors with typed `ErrorCode`.
* [x] **Non-destructive API**: Implement `clone()` behavior for `toBuffer` (ADR-001).

### ⚡ Optimization & Efficiency (v0.8.x - Completed)

* [x] **WebP Speed Tuning**: Optimize default parameters (Method 6 → 4) to match `sharp`. ✅ **Completed in v0.8.1**

### ⚡ Optimization & Efficiency (v0.9.x - In Progress)
* [x] **Strip Metadata by Default**: Remove Exif/XMP for security and smaller file sizes. ✅ **Completed in v0.9.x** (GPS auto-strip, EXIF preservation opt-in)
* [ ] **Binary Size Reduction**: Enable LTO and strip symbols for faster cold starts.
* [ ] **Documentation Update**: Publish "True Benchmarks" (AVIF speed / JPEG size).

### 🛡️ Reliability & Stability (v1.0.0 - Planned)

* [ ] **Fuzzing Tests**: Implement `cargo-fuzz` to prevent crashes from malformed inputs.
* [ ] **Memory Leak Detection**: CI integration with Valgrind/Sanitizers.
* [ ] **API Freeze**: Final review of TypeScript definitions and public Rust API.

---

## 📅 Detailed Version Roadmap

### v0.8.1 - "WebP Speed Optimization" (Released 2026-01-01)

**Focus:** WebP encoding performance parity with sharp.

* **Performance: WebP Optimization** ✅
* **Goal:** Eliminate the "5x slower than sharp" bottleneck. ✅ **Achieved**
* **Task:** Change default WebP `method` from 6 to 4. ✅ **Completed**
* **Task:** Disable heavy preprocessing by default. ✅ **Completed**
* **Result:** ~4x speed improvement, now matches sharp's performance while maintaining quality parity.

### v0.9.0 - "The Optimizer" (Next Release)

**Focus:** Winning the benchmarks in all categories (Speed & Size).


* **Efficiency: Secure-by-Default Metadata** ✅ **Completed**
* **Goal:** Ensure output files are smaller than `sharp`'s and privacy-safe.
* **Task:** ~~Implement logic to strip Exif, XMP, and Comments during encoding.~~ Done
* **Task:** ~~Add `.keepMetadata()` API for opt-in preservation.~~ Done (with GPS auto-strip)


* **Deployment: Binary Minimization**
* **Goal:** Reduce binary size from ~9MB to <7MB for AWS Lambda.
* **Task:** Configure `Cargo.toml` with `lto = "fat"`, `strip = "symbols"`, `codegen-units = 1`.



### v1.0.0 - "Production Ready" (Stable)

**Focus:** proving reliability for enterprise adoption.

* **Security: Automated Fuzzing**
* **Goal:** Zero panic/crashes on corrupted inputs.
* **Task:** Create fuzz targets for the decoder pipeline.


* **Stability: Long-running Tests**
* **Goal:** Prove memory safety across NAPI boundaries over time.
* **Task:** Add memory leak check jobs to GitHub Actions.


* **API: Final Freeze**
* **Goal:** Guarantee no breaking changes for v1.x lifecycle.
* **Task:** Audit `index.d.ts` and verify all public interfaces.



### v1.x - "Serverless Native" (Future)

**Focus:** Advanced cloud-native features.

* **Smart Concurrency (Auto Memory Cap)**
* **Goal:** Prevent OOM kills in constrained containers (e.g., 512MB limit).
* **Task:** Detect container memory limits and auto-adjust thread pool size.


* **Telemetry Hooks**
* **Task:** Expose detailed metrics (CPU time, Peak RAM) per request.



### v2.0 - "Universal Engine" (Long Term)

**Focus:** Beyond Node.js.

* **WebAssembly (Wasm) Support**: Support for Cloudflare Workers and Browsers.
* **Streaming API**: Native support for Web Streams API.

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

本ドキュメントは `lazy-image` の開発状況と将来の方向性を定義します。本プロジェクトはセマンティックバージョニングに従います。

## ✅ 達成状況チェックリスト

### 🏗️ アーキテクチャとコア (v0.8.x - 完了)

* [x] **真の遅延読み込み (True Lazy)**: `fromPath` によるファイルIOの遅延化。
* [x] **ゼロコピー・アーキテクチャ**: フォーマット変換時のピクセルコピー回避。
* [x] **スレッド安全性**: libuv/rayon スレッドプールの競合解消。
* [x] **構造化エラーハンドリング**: 文字列エラーの撤廃と `ErrorCode` の導入。
* [x] **非破壊API**: `toBuffer` の `clone()` 挙動の確定 (ADR-001)。

### ⚡ 最適化と効率性 (v0.9.x - 進行中)

* [x] **WebP 速度チューニング**: デフォルト設定を最適化し `sharp` と同等の速度へ。 ✅ **v0.8.1 で完了**
* [x] **メタデータ削除のデフォルト化**: Exif/XMPを削除し、セキュリティとサイズを改善。 ✅ **v0.9.x で完了** (GPS自動削除、EXIF保持オプトイン)
* [ ] **バイナリサイズ削減**: LTO有効化によるコールドスタート高速化。
* [ ] **ドキュメント更新**: 「真実のベンチマーク」（AVIF速度/JPEGサイズ）の公開。

### 🛡️ 信頼性と安定性 (v1.0.0 - 計画中)

* [ ] **ファジングテスト**: 不正な入力データによるクラッシュ防止。
* [ ] **メモリリーク検知**: Valgrind/Sanitizer によるCIテスト導入。
* [ ] **API 凍結**: TypeScript定義とRust公開APIの最終確定。

---

## 📅 バージョン別詳細ロードマップ

### v0.9.0 - "The Optimizer" (次回リリース)

**目標:** ベンチマークにおける弱点（WebPの速度・ファイルサイズ）を完全に克服する。

* **Performance: WebP 最適化**
* **ゴール:** 「sharpより5倍遅い」状態の解消。
* **タスク:** デフォルトの `method` を 6 から 4 へ変更。
* **タスク:** 重い前処理（preprocessing）をデフォルトで無効化。


* **Efficiency: セキュア・デフォルト (メタデータ削除)** ✅ **完了**
* **ゴール:** `sharp` よりも出力ファイルを小さくし、プライバシーを保護する。
* **タスク:** ~~エンコード時に Exif, XMP, Comments を自動削除するロジックの実装。~~ 完了
* **タスク:** ~~`.keepMetadata()` API（オプトイン機能）の追加。~~ 完了 (GPS自動削除付き)


* **Deployment: バイナリ最小化**
* **ゴール:** AWS Lambda 向けにバイナリサイズを ~9MB から 7MB以下へ。
* **タスク:** `Cargo.toml` にて `lto = "fat"`, `strip = "symbols"` 等を適用。



### v1.0.0 - "Production Ready" (安定版)

**目標:** 企業採用に耐えうる「信頼性」の証明。

* **Security: 自動ファジング**
* **ゴール:** 破損した画像によるパニック発生率ゼロ。
* **タスク:** デコーダーに対する `cargo-fuzz` ターゲットの作成。


* **Stability: 長時間稼働テスト**
* **ゴール:** NAPI境界におけるメモリ安全性の証明。
* **タスク:** GitHub Actions にメモリリーク検知ジョブを追加。


* **API: 完全凍結**
* **ゴール:** v1.x 系における破壊的変更なしの保証。
* **タスク:** `index.d.ts` の全監査とインターフェース確定。



### v1.x - "Serverless Native" (将来)

**目標:** クラウドネイティブ機能の強化。

* **スマート・コンカレンシー (自動メモリ制御)**
* **ゴール:** 低メモリコンテナ（例: 512MB）での OOM Kill 回避。
* **タスク:** コンテナのメモリ制限を検知し、スレッドプールサイズを自動調整。


* **テレメトリー**
* **タスク:** リクエストごとのCPU時間やピークメモリ使用量の取得API。



### v2.0 - "Universal Engine" (長期)

**目標:** Node.js の枠を超える。

* **WebAssembly (Wasm) 対応**: Cloudflare Workers やブラウザでの動作。
* **ストリーミング API**: Web Streams API のネイティブサポート。

---

## 🚫 やらないこと (Non-Goals)

プロジェクトの焦点と安定性を維持するため、以下の機能は明確に**スコープ外**とします。

1. **描画・合成**: テキスト描画、ウォーターマーク、図形描画など。
2. **複雑なフィルタ**: ぼかし、シャープネス、エンボス加工など。
3. **動画・アニメーション**: GIF/APNG の作成や編集。
4. **レガシーサポート**: 32bit OS や EOL を迎えた Node.js のサポート。