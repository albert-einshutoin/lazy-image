# lazy-image 🦀 (日本語サマリー)

英語版 README が正です。このファイルは主要ポイントの日本語サマリーです。詳細・最新情報は必ず [README.md](./README.md) を参照してください。

## Quick Start (5 lines)

```javascript
const { ImageEngine } = require('@alberteinshutoin/lazy-image');

const bytesWritten = await ImageEngine.fromPath('input.png')
  .resize(800)
  .toFile('output.jpg', 'jpeg', 80);

console.log(`Wrote ${bytesWritten} bytes`);
```

英語版では簡潔なクイックスタートと README 構成の要点を先に示しており、同じ順序を維持しています。

## Choose lazy-image if / Choose sharp if

**採用したい場合**
- バンドル帯域を節約したい、CDN 配信コストを抑えたい
- 入力ファイルを大きめに扱うサーバーレスやコンテナでメモリ上限が重要

**使い分け**
- full API の drop-in が必要、または複雑なフィルタ/ドローイングが中心なら sharp

## Recommended Paths

- Web 配信最適化: `fromPath()` → `resize()` → `toFile()`
- ビルド時バッチ: `processBatch()` を中心に利用
- 入稿画像の安全性担保: `sanitize({ policy: 'strict' })`

## Cost Savings Example (ROI)

1MB 画像を月 10M 回配信するケースでは、平均 15% 近い縮小差で月間数百ドル規模の帯域削減を見込めます。詳細は英語版の ROI セクションと
[docs/ROI_CALCULATOR.md](./docs/ROI_CALCULATOR.md) を参照してください。

## Installation

```bash
npm install @alberteinshutoin/lazy-image
```

| 配布経路 | 補足 |
|---|---|
| npm optional binaries | macOS arm64/x64、Linux x64/arm64 GNU、Linux x64 musl、Windows x64 |
| ソースビルド | Node.js 18+、Rust 1.88+、ネイティブ codec toolchain が必要 |

## Architecture Overview

- `ImageEngine` は「構成 → キュー投入 → エンコード実行」の遅延実行モデル
- 小〜中サイズ画像は V8 ヒープ経由を避け、ネイティブ側の Rust バッファ優先の処理
- メタデータはデフォルトで除去、`keepMetadata()` で明示維持

## Basic Usage

- `fromPath()` でファイルを読み込む
- `.resize()`, `.rotate()`, `.grayscale()` などを連鎖
- `toBuffer()` / `toFile()` で JPEG/WebP/AVIF/PNG へ変換

## Documentation

- API: [docs/API.md](./docs/API.md)
- 受け入れ条件・非ゴール: [docs/PROJECT_PHILOSOPHY.md](./docs/PROJECT_PHILOSOPHY.md)
- 性能・互換: [docs/PERFORMANCE.md](./docs/PERFORMANCE.md), [docs/COMPATIBILITY.md](./docs/COMPATIBILITY.md)

## Features (summary)

- AVIF/WebP/JPEG/PNG エンコード
- Image Firewall とメタデータ安全性（`keepMetadata()`）
- ファイルパス入力のメモリ特性最適化
- Rust コア (napi-rs) + Node.js API

## Development

```bash
npm install && npm run build
npm test
```

コントリビュート手順は [CONTRIBUTING.md](./CONTRIBUTING.md) を参照。

## License

MIT

## Credits

[mozjpeg](https://github.com/mozilla/mozjpeg), [libwebp](https://chromium.googlesource.com/webm/libwebp), [libavif](https://github.com/AOMediaCodec/libavif), [napi-rs](https://napi.rs/)
