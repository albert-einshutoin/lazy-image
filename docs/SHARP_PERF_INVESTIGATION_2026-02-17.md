# sharp 比較パフォーマンス徹底調査 (2026-02-17)

## 1. 実施体制 (gitflow + worktree)
- Base: `develop`
- Investigation branch: `codex/feature-sharp-perf-investigation`
- Worktree: `/Users/shutoide/Developer/perf-sharp-investigate`

## 2. 結論サマリ
- 遅さの主因は `decode` や `resize` ではなく **`encode` 段**。
- 特に `WebP` と `AVIF` が `sharp` 比で遅く、`JPEG` はほぼ同等か勝てる条件がある。
- 現行 `release opt-level = "s"` は速度面で不利。`opt-level = 3` で実測改善。
- ただし WASM 想定ではサイズ最適化が必要なため、`release`（Node向け）と `release-wasm`（WASM向け）を分離運用する。
- AVIF を高速側に振ると速度は大幅改善するが、サイズ優位性は悪化。
- 品質判定は「原画像との距離」ではなく「sharp 出力との距離」なので、失敗理由の解釈に注意。
- `lazy-image` の真価は「最速」単独ではなく、**メモリ安全性/OOM耐性 + 運用制御 + 十分な性能**の総合値にある。

## 3. 再現ベンチ結果

### 3.1 既存ベンチ (`npm run test:bench:compare`)
- JPEG: 102ms vs 82ms (0.80x), サイズ -20.0%
- WebP: 172ms vs 83ms (0.48x), サイズ +9.3%
- AVIF: 549ms vs 192ms (0.35x), サイズ +29.0%
- Complex: 83ms vs 117ms (1.41x), サイズ +10.1%

### 3.2 多回平均 (8回, `toBufferWithMetrics`) ベースライン
(ローカルビルド, `opt-level = "s"`)
- JPEG q80
  - lazy 69.65ms / sharp 71.11ms / speed_ratio 1.021
  - decode 36.17ms / ops 13.68ms / encode 19.18ms
- WebP q80
  - lazy 174.97ms / sharp 77.95ms / speed_ratio 0.446
  - decode 37.23ms / ops 13.42ms / encode 123.89ms
- AVIF q60
  - lazy 542.55ms / sharp 170.75ms / speed_ratio 0.315
  - decode 37.04ms / ops 13.84ms / encode 491.27ms

=> `WebP/AVIF` は encode が支配的。

## 4. A/B 実験

### 4.1 `profile.release.opt-level: "s" -> 3`
- JPEG q80: 69.65ms -> 64.42ms (改善)
- WebP q80: 174.97ms -> 168.20ms (小改善)
- AVIF q60: 542.55ms -> 467.92ms (大改善)

所見:
- サイズ最適化中心のビルド設定が速度を削っていた。
- 特に AVIF で改善幅が大きい。

### 4.2 WebP: `thread_level=1` 追加
- WebP q80: 168.20ms -> 169.05ms (ほぼ不変)

所見:
- thread_level 単体では有意改善なし。

### 4.3 AVIF: `speed=10`, `encoder_threads=4` (実験値)
- AVIF q60: 467.92ms -> 222.13ms (大幅改善)
- ただしサイズ差: +29.0%相当 -> +36.3%相当に悪化
- `lazy vs sharp` 品質差指標: SSIM 0.9928 / PSNR 39.47dB (依然ゲート未達)

所見:
- AVIF は速度と圧縮効率のトレードオフが極端。
- 「高速化のみ」を狙うと lazy-image の強み（サイズ優位）を削る。

## 5. なぜ sharp に負けるか (根因)
1. WebP/AVIF の encode 実装特性差
- `sharp` は libvips 経由で実運用向けチューニングが進んだ高速経路。
- lazy-image はサイズ最適化寄りの設計が混在し、速度最適点が異なる。

2. リリースビルド方針
- `opt-level = "s"` は現実的に encode hot path に不利。

3. 品質ゲートの評価基準
- sharp 出力への一致度を基準化しており、コーデック差そのものが失敗要因化。

## 6. lazy-image の強み
1. JPEG 圧縮効率
- 同品質帯でサイズ優位を取りやすい (今回も -20%)。

2. 複合処理でのスループット余地
- Complex pipeline で sharp 同等〜優位が観測。

3. 安全性/制御性
- Firewall, metadata 制御, Rust 側エラーモデルが強い。

4. 目的特化
- 「最低レイテンシ」より「配信サイズ最適化」を重視するワークロードに適合。

## 6.1 メモリ安全性/OOM耐性の実測と意味付け
本調査では、速度だけでなく「小さなサーバーで落ちずに回る」ことを `lazy-image` の中核価値として扱う。

### 実測1: メモリトラッキングベンチ (`npm run test:bench:memory`)
- 入力: `test_4.5MB_5000x5000.png`
- シナリオ平均（6 runs, warmup 1）
  - `jpeg-noops-q80`: Avg RSS delta `7.4MB`, Avg metrics.peakRss `327.6MB`, Avg time `635.6ms`
  - `webp-resize800-q80`: Avg RSS delta `5.2MB`, Avg metrics.peakRss `377.9MB`, Avg time `160.0ms`
  - `jpeg-resize-rotate-gray-q75`: Avg RSS delta `7.5MB`, Avg metrics.peakRss `430.2MB`, Avg time `89.4ms`

所見:
- `rss delta` は小さく抑制されており、処理中ピークは `metrics.peakRss` で監視可能。
- 速度改善を行う際も、`peakRss` を同時監視しないと `lazy-image` の価値を毀損する。

### 実測2: Zero-copy 計測 (`node --expose-gc docs/scripts/measure-zero-copy.js`)
- `heap_delta_mb: 0`
- `rss_delta_mb: 34.6`
- `peak_rss_metrics_mb: 169`

所見:
- V8 ヒープ膨張を抑え、OSメモリ管理側に寄せる設計意図と整合。
- Nodeワーカー多重時に、ヒープ起因GCスパイクを避けやすい。

### 実測3: OOM/境界系の安全テスト (`node test/integration/edge-cases.test.js`)
- 結果: `31 passed, 0 failed`
- oversized/invalid/empty 入力、過大並列、極端パラメータを拒否または安全処理できることを確認。

### 方針
- `sharp` と同等以上を目指す指標は「速度のみ」ではなく以下の複合KPIにする。
  1. Throughput (`total_ms`, `encode_ms`)
  2. Memory safety (`peakRss`, `rss delta`, OOM/境界入力テスト通過率)
  3. Output value (size diff, quality)

## 7. 何を変えれば良いか (優先順位)

### P0 (すぐやる)
1. `profile.release.opt-level` を速度重視へ変更
- 候補: `3` または `z`/`s`とのハイブリッドを再設計。
- まずは `3` で CI ベンチ回帰を測る。
 - 併せて `release-wasm` を用意し、WASM では `opt-level = "s"` を維持する。

2. ベンチ評価軸を二層化
- `sharp一致` と `原画像基準` を分離。
- 失敗判定を「一致度」だけに依存させない。

3. AVIF をモード化
- `size-first` と `speed-first` のプリセットを分ける。
- デフォルトは現状維持、用途別に明示選択可能にする。

### P1 (次にやる)
1. WebP パラメータ探索の自動化
- method/pass/filter/sns の探索ベンチを導入。
- 「速度改善 >= X% かつ サイズ悪化 <= Y%」の制約探索にする。

2. AVIF backend 方針の明確化
- rav1e 固定で行くか、用途別 backend 戦略を取るかを決定。

3. README/Docs の期待値を更新
- 「何が速く、何が遅いか」をフォーマット別に明記。
 - 「速度」だけでなく「最小メモリ環境での安定運用」を選定軸として明記。

### P2 (中期)
1. プリセットAPI強化
- `toBuffer(format, quality, profile?)` 的に `profile: speed|balanced|size` を導入。

2. 継続ベンチ運用
- PR ごとに WebP/AVIF の encode_ms を時系列監視。

## 8. プロダクト戦略としての整理
- sharp の土俵（汎用高速処理）で全面対抗するのは非効率。
- lazy-image は以下に集中すべき。
  - JPEG/配信サイズ最適化
  - セキュリティ/運用制御（Firewall/metadata）
  - 複合処理での安定性能
- AVIF/WebP は「絶対最速」ではなく「目的別プリセット」で勝つ。

## 8.1 WASM化を見据えたビルド戦略（価値を崩さないための分離）
現時点で WASM を本番提供していなくても、最適化方針は先に分離しておく。

- Node/NAPI（現行主戦場）: `profile.release` を速度優先 (`opt-level = 3`)
- WASM（将来）: `profile.release-wasm` をサイズ優先 (`opt-level = "s"`)

この分離により、
- 現在の速度改善を阻害せず、
- 将来の配布サイズ要件（WASM）にも後戻りなく対応できる。

## 9. 現時点の推奨アクション
1. `opt-level=3` を暫定採用し、CI ベンチで regressions を確認。
2. AVIF に `speed-first` プリセットを追加 (デフォルトは現行)。
3. ベンチ評価を `sharp一致` + `原画像基準` の2軸に分離。
4. WebP は thread_level 以外の本命パラメータ探索へ移行。
5. CI の必須監視に `peakRss` と OOM/境界テスト通過を追加し、「速いが落ちる」最適化を禁止する。

## 10. 真価を見失わないためのガードレール
`lazy-image` の改善は、以下を同時に満たすものだけ採用する。

1. 速度: 対象シナリオで有意改善（例: `encode_ms` 改善）
2. 安全: OOM/境界系テストの退行なし
3. メモリ: `peakRss` の悪化が許容範囲内
4. 価値: 出力サイズ優位または運用制御価値（Firewall/metadata）を毀損しない

このルールにより、「sharp 追従のために lazy-image らしさを失う」ことを防ぐ。
