# lazy-image 🦀 (日本語サマリー)

英語版 README が正です。このファイルは主要ポイントの日本語サマリーです。詳細・最新情報は必ず [README.md](./README.md) を参照してください。

- `lazy-image` は Web 画像最適化に特化した Node.js エンジンです。
- `sharp` のドロップイン代替ではありません。
- 現行ベンチマークは PNG → JPEG の2つの代表シナリオ（変換・800px リサイズ）で、`sharp` より画像サイズが小さくなるケースがあります。
- セキュリティ優先: メタデータ（EXIF/ICC）は既定で除去され、`keepMetadata()` で明示的に保持。XMP は現状保持されません。`Image Firewall` で入力を検証。
- V8 ヒープを経由しない入力パス（`fromPath()`）を採用し、256MB 以下は Rust 所有バッファ、256MB 超は advisory lock 付き mmap を使います。処理中に対象ファイルを更新・トリム・削除すると破損や失敗に繋がるため、mutable な更新ワークフローではコピー経由の API を使ってください。

## Quick Start (5 lines)

```javascript
const { ImageEngine } = require('@alberteinshutoin/lazy-image');

const bytesWritten = await ImageEngine.fromPath('input.png')
  .resize(800)
  .toFile('output.jpg', 'jpeg', 80);

console.log(`Wrote ${bytesWritten} bytes`);
```

## Choose lazy-image if / Choose sharp if

| **Choose lazy-image if** | **Choose sharp if** |
|---|---|
| 帯域コストが重要 | 速度優先で最大スループットが必要 |
| サーバレス/メモリ制約環境 | 高機能編集 API が必要 |
| 安全な既定値が重要（Image Firewall） | 完全互換 API が必要 |
| PNG→JPEG の体感でサイズ最適化が必要 | GIF / SVG / TIFF など広範なフォーマットを必要とする |

## Recommended Paths

- **Web 配信最適化**: `fromPath() -> resize()/crop() -> toFile()`
- **アップロード検証**: `fromPath() -> sanitize({ policy: 'strict' }) -> toFile()/toBuffer()`
- **バッチ生成**: `processBatch()` / `clone()`

## Cost Savings Example (ROI)

本体価格最適化は画像数・サイズと配信回数で変わるため、実運用に合わせて [docs/roi-calculator.html](./docs/roi-calculator.html) を参照してください。

## Installation

```bash
npm install @alberteinshutoin/lazy-image
```

`npm` の optional binaries（macOS / Linux / Windows）が入り、`README.md` の最新手順に従ってビルド可能です。

## Architecture Overview

`lazy-image` は Rust NAPI コアと軽量な JS バインディングで、
- 入力検証・最適化パイプライン
- メトリクス付き変換実行
- メモリ上限を意識した並列制御

を合わせた構成で、`src/`（Rust）と `lib/`（JS 補助）で分離しています。

## Basic Usage

**リサイズ + 保存**

```javascript
await ImageEngine.fromPath('photo.jpg')
  .resize({ width: 800, fit: 'inside' })
  .toFile('thumb.jpg', 'jpeg', 85);
```

**フォーマット変換**

```javascript
const buffer = await ImageEngine.fromPath('input.png')
  .resize({ width: 600 })
  .toBuffer('webp', 80);
```

## Documentation

- **概要**: [README.md](./README.md)
- **API 参照**: [docs/API.md](./docs/API.md)
- **仕様/アーキテクチャ**: [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md)
- **互換性・性能**: [docs/COMPATIBILITY.md](./docs/COMPATIBILITY.md), [docs/PERFORMANCE.md](./docs/PERFORMANCE.md)
- **セキュリティ**: [SECURITY.md](./SECURITY.md)

## Features (summary)

smaller JPEG (mozjpeg) · WebP (libwebp) · AVIF (libavif) · メタデータ制御 · メモリ制御を前提にした API · Rust コア + NAPI-RS。

## Development

```bash
npm install && npm run build
npm test
```

`CONTRIBUTING.md` の手順に従って運用します。

## License

MIT

## Credits

[mozjpeg](https://github.com/mozilla/mozjpeg) · [libwebp](https://chromium.googlesource.com/webm/libwebp) · [libavif](https://github.com/AOMediaCodec/libavif) · [fast_image_resize](https://github.com/Cykooz/fast_image_resize) · [img-parts](https://github.com/paolobarbolini/img-parts) · [napi-rs](https://napi.rs/)
