# lazy-image 🦀 (日本語サマリー)

英語版 README.md が正本です。詳細は必ず [README.md](./README.md) を優先してください。

## Quick Start (5 lines)

```javascript
const { ImageEngine } = require('@alberteinshutoin/lazy-image');

const bytesWritten = await ImageEngine.fromPath('input.png')
  .resize(800)
  .toFile('output.jpg', 'jpeg', 80);

console.log(`Wrote ${bytesWritten} bytes`);
```

## Choose lazy-image if / Choose sharp if

| **lazy-image を選ぶ場合** | **sharp を選ぶ場合** |
|---|---|
| CDN / 画像配信最適化の帯域節約を優先 | 最大スループットや API 網羅が必要 |
| serverless・メモリ制約環境で運用 | 描画・合成・高度な画像編集 API が必要 |
| 既定で安全側寄りの処理を優先 | 既存 sharp エコシステムとの完全互換が必要 |
| JPEG サイズ最適化を重視 | GIF / SVG / TIFF などが前提で必要 |

**差分が出る主要点**

- JPEG サイズは同じ encoder quality 設定の canonical な PNG→JPEG 2ケースで 17-20% 小さい。resize ケースは source-reference 品質下限を通過しますが、完全な知覚品質一致を意味しません
- 256MB を超える大きな入力は Rust 側バッファへ全量読み込まず、メモリ安全な mmap 経路で処理します。`fromPath()` を使う際は、同時に対象ファイルの変更・切り詰め・削除を避け、破損や `SIGBUS` / `SIGSEGV` を防いでください。
- `fromPath()` は256MB以下のsourceを呼び出しthreadで同期readします。HTTP/serverless経路では `await ImageEngine.fromPathAsync(path)` を使い、source setupをNode.js event loop外へ移してください。
- メタデータは既定で安全寄り（GPS は既定で除去、`keepMetadata()` で制御）
- API は drop-in 置換ではない（互換が必要なら sharp）

**プロジェクト方針**
- 目的は smaller web payload + bounded memory + 安全デフォルト
- 「速い」「常に軽量」は断定しない。ワークロード別検証が必要

## Architecture Overview

`lazy-image` は Rust コア + Node.js バインディングで、入力・変換・エンコードを lazy 実行します。詳細は [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) を参照してください。

## Recommended Paths

- 画像配信最適化: `await fromPathAsync() -> resize()/crop() -> toFile()`
- アップロード検証: `await fromPathAsync() -> sanitize({ policy: 'strict' }) -> toFile()/toBuffer()`
- 静的サイト生成バッチ: `processBatch()` / `clone()`
- 編集後の最終最適化: sharp/ImageMagick の後段に lazy-image を通す

## Cost Savings Example (ROI)

README（英語版）と同じ想定計算を参照します。月間配信量とエンコーディング回数次第で節約は拡大します。

## Installation

```bash
npm install @alberteinshutoin/lazy-image
```

| 環境 | 概要 |
|---|---|
| ランタイム | Node.js 22+。platform optional dependencies が自動インストール |
| パッケージサイズ | プラットフォーム別で 6〜9MB 前後 |
| 自前ビルド | `npm run build` |

## Basic Usage

```javascript
await ImageEngine.fromPath('photo.jpg')
  .resize({ width: 800, fit: 'inside' })
  .toFile('thumb.jpg', 'jpeg', 85);
```

```javascript
const { inspectFile } = require('@alberteinshutoin/lazy-image');
const meta = inspectFile('input.jpg');
```

## Documentation

英語版の構成と同じです。`docs/`, `examples/`, `spec/` を順に確認してください。
選択的テストCIの判定方法、安全側の全件フォールバック、ローカル再現手順は
[docs/SELECTIVE_TESTING.md](./docs/SELECTIVE_TESTING.md) を参照してください。

## Features (summary)

- JPEG/PNG/WebP/AVIF エンコード
- ICC / EXIF / GPS オフロード
- フォーマット別最適化・メトリクス
- Image Firewall / Rust メモリ安全

## Development

```bash
npm install && npm run build
npm test
```

## License

MIT

## Credits

[mozjpeg](https://github.com/mozilla/mozjpeg) · [libwebp](https://chromium.googlesource.com/webm/libwebp) · [libavif](https://github.com/AOMediaCodec/libavif) · [fast_image_resize](https://github.com/Cykooz/fast_image_resize) · [img-parts](https://github.com/paolobarbolini/img-parts) · [napi-rs](https://napi.rs/)
