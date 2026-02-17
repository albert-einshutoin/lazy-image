# sharp 比較パフォーマンス徹底調査 (2026-02-17)

## 1. 実施体制 (gitflow + worktree)
- Base: `develop`
- Investigation branch: `codex/feature-sharp-perf-investigation`
- Worktree: `/Users/shutoide/Developer/perf-sharp-investigate`

## 2. 結論サマリ
- 遅さの主因は `decode` や `resize` ではなく **`encode` 段**。
- 特に `WebP` と `AVIF` が `sharp` 比で遅く、`JPEG` はほぼ同等か勝てる条件がある。
- 現行 `release opt-level = "s"` は速度面で不利。`opt-level = 3` で実測改善。
- AVIF を高速側に振ると速度は大幅改善するが、サイズ優位性は悪化。
- 品質判定は「原画像との距離」ではなく「sharp 出力との距離」なので、失敗理由の解釈に注意。

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

## 7. 何を変えれば良いか (優先順位)

### P0 (すぐやる)
1. `profile.release.opt-level` を速度重視へ変更
- 候補: `3` または `z`/`s`とのハイブリッドを再設計。
- まずは `3` で CI ベンチ回帰を測る。

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

## 9. 現時点の推奨アクション
1. `opt-level=3` を暫定採用し、CI ベンチで regressions を確認。
2. AVIF に `speed-first` プリセットを追加 (デフォルトは現行)。
3. ベンチ評価を `sharp一致` + `原画像基準` の2軸に分離。
4. WebP は thread_level 以外の本命パラメータ探索へ移行。
