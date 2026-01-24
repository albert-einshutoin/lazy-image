# lazy-image 🦀 (日本語サマリー)

英語版 README が正です。このファイルは主要ポイントの日本語サマリーです。詳細・最新情報は必ず [README.md](./README.md) を参照してください。

## 位置付け
- Web 向け画像最適化に特化した意図的なエンジン
- まだ sharp のドロップイン代替ではありません（互換 API が必要なら sharp を使用）
- セキュリティ優先: EXIF/XMP などのメタデータはデフォルトで削除、ICC は色精度のため保持
- ゼロコピー入力パス: `fromPath()/processBatch()` → `toFile()` で入力ファイルを JS ヒープへコピーしない
- AVIF の ICC は v0.9.x（libavif-sys）で保持。<0.9.0 や ravif のみ構成では破棄

## 計測可能な指標
- JS ヒープ増加: `fromPath → toBufferWithMetrics` で **2MB 以下**（`node --expose-gc docs/scripts/measure-zero-copy.js` で検証）
- RSS 目安: `peak_rss ≤ decoded_bytes + 24MB`（decoded_bytes = width × height × bpp; JPEG bpp=3, PNG/WebP/AVIF bpp=4）。例: 6000×4000 PNG (≈96MB) → 目標 RSS ≤ 120MB
- 品質ゲート: SSIM ≥ 0.995 / PSNR ≥ 40dB （sharp 出力と比較するベンチで検証）

## 主な特徴
- AVIF/WebP/JPEG/PNG エンコード（AVIFは高速かつ小サイズ）
- ICC プロファイル保持（AVIFは v0.9.x 以降）
- EXIF 自動回転（`autoOrient(false)` で無効化）
- ディスクバッファ型ストリーミングでメモリを O(1) 近傍に抑制
- Rust コアによるメモリ安全 & Node.js バインディング (napi-rs)

## 推奨シナリオ
- CDN/モバイル向けの帯域削減
- バッチ処理・静的サイト生成
- メモリ制約環境（512MB コンテナなど）
- AVIF 生成・色精度重視ワークフロー

## セキュリティと互換性
- 安全策の詳細とサポートバージョン: [SECURITY.md](./SECURITY.md)
- 互換性のマトリクスと移行ノート: [docs/COMPATIBILITY.md](./docs/COMPATIBILITY.md)

---
英語版で更新が先行します。疑問点や翻訳改善の提案は Issue/PR で歓迎します。
