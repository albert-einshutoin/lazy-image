# lazy-image 🦀 (日本語サマリー)

英語版 README が正です。このファイルは主要ポイントの日本語サマリーです。詳細・最新情報は必ず [README.md](./README.md) を参照してください。

## 位置付け
- Web 向け画像最適化に特化した意図的なエンジン
- まだ sharp のドロップイン代替ではありません（互換 API が必要なら sharp を使用）
- 現行の正本ベンチマークでは、PNG → JPEG の単純な 2 シナリオ（no-resize 変換 / 800px resize）で sharp より **17-20% 小さい JPEG** を生成。AVIF/WebP と複合操作 JPEG pipeline の速度・サイズはワークロード依存で、現行測定では sharp が有利なケースがあります
- セキュリティ優先: 全メタデータ（EXIF/XMP/ICC）をデフォルトで削除。`keepMetadata()` で保持可能
- V8 ヒープを経由しない入力パス: `fromPath()/processBatch()` → `toFile()` で入力ファイルを JS ヒープへコピーしない（256 MB 以下は Rust 所有バッファに読み込み、256 MB 超は advisory lock 付き mmap）
- AVIF の ICC は v0.9.x（libavif-sys）で保持。古い/別構成の AVIF backend では破棄される場合があります
- `lazy` の意味: [docs/LAZY_SEMANTICS.md](./docs/LAZY_SEMANTICS.md) を参照（constructor は無作業ではない）
- 思想と非目標: [docs/PROJECT_PHILOSOPHY.md](./docs/PROJECT_PHILOSOPHY.md)

## 計測可能な指標
- JS ヒープ増加: `fromPath → toBufferWithMetrics` で **2MB 以下**（`node --expose-gc docs/scripts/measure-zero-copy.js` で検証）
- RSS 目安: `peak_rss ≤ decoded_bytes + 24MB`（decoded_bytes = width × height × bpp; JPEG bpp=3, PNG/WebP/AVIF bpp=4）。例: 6000×4000 PNG (≈96MB) → 目標 RSS ≤ 120MB
- 品質比較: `npm run test:bench:compare` は sharp 出力との SSIM/PSNR を出力。公開性能 claim の正本は [docs/TRUE_BENCHMARKS.md](./docs/TRUE_BENCHMARKS.md)

## 主な特徴
- AVIF/WebP/JPEG/PNG エンコード（AVIF/WebP の速度・サイズ優位はワークロードごとに要検証）
- ICC/EXIF 保持オプション（AVIFの ICC は v0.9.x 以降、GPS は自動削除）
- EXIF 自動回転（`autoOrient(false)` で無効化）
- ディスクバッファ型ストリーミングでメモリを O(1) 近傍に抑制
- Rust コアによるメモリ安全 & Node.js バインディング (napi-rs)

## 推奨シナリオ
- CDN/モバイル向けの JPEG 帯域削減
- バッチ処理・静的サイト生成
- メモリ制約環境（512MB コンテナなど）
- AVIF 生成・色精度重視ワークフロー（サイズ/速度は対象画像で検証）

## セキュリティと互換性
- 安全策の詳細とサポートバージョン: [SECURITY.md](./SECURITY.md)
- 互換性のマトリクスと移行ノート: [docs/COMPATIBILITY.md](./docs/COMPATIBILITY.md)

---
英語版で更新が先行します。疑問点や翻訳改善の提案は Issue/PR で歓迎します。
