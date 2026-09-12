# lazy-image

Node.js のアップロード処理やビルド工程で、公開用の画像一式を生成・検証するライブラリです。
policy から複数サイズの画像、任意の placeholder、manifest を作り、検証後にローカルへ確定します。
単画像の変換・最適化には `ImageEngine` を使えます。

[English](./README.md) · [ドキュメント一覧](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/README.md)

## 用途を選ぶ

| 目的 | 入口 |
|---|---|
| 公開用画像一式と manifest を作る | [導入ガイド](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/ADOPTION_GUIDE.md) |
| 単画像を最適化する | 下のクイックスタート |
| 採用判断・他製品との違いを知る | [競合比較](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/COMPETITIVE_ANALYSIS.md) |

認証、アップロード受付、ジョブ管理、ストレージ転送、URL 配信はアプリケーションが担当します。

## インストール

Node.js 22+。対応環境では native binary を自動インストールします。

```bash
npm install @alberteinshutoin/lazy-image
```

## クイックスタート

`.mjs` ファイルで実行する例です。

```javascript
import { ImageEngine } from '@alberteinshutoin/lazy-image';

const image = await ImageEngine.fromPathAsync('input.png');
await image.resize({ width: 800, fit: 'inside' }).toFile('output.jpg', 'jpeg', 80);
```

CommonJS の `require()` にも対応します。サーバーでは `fromPathAsync()` を使います。

## よく使う処理

### 公開用画像一式を作る

```javascript
import { compileImage } from '@alberteinshutoin/lazy-image';

const manifest = await compileImage({
  inputPath: '/srv/uploads/image.bin',
  outputDir: '/srv/public/images/v1',
  policy: { widths: [320, 640], formats: ['webp'], placeholder: true },
});
```

出力先は未存在のディレクトリです。親ディレクトリは信頼でき、並行して置換されず、
staging と同じ filesystem 上にある必要があります。失敗した処理の成果物は公開しません。
APIでは追加のcleanup失敗を `error.cleanupError` で確認できます。CLIはこの付随エラーを表示しないため、
stderrだけでは未公開 staging の削除完了を確認できません。

### CLI

`policy.json` を作ります。

```json
{"widths":[320,640],"formats":["webp"],"placeholder":true}
```

```bash
npx @alberteinshutoin/lazy-image compile input.jpg \
  --out-dir public/images/v1 --policy policy.json
```

成功時の stdout は manifest JSON、診断は stderr。終了コードは 0 が成功、
2 が引数・policy JSON の不正、1 が compiler 実行失敗です。

## 主なAPI

`ImageEngine` は resize、crop、rotate、flip、grayscale、sanitize と各出力メソッドを提供します。
複数出力には clone、responsive helpers、batch API も利用できます。
一般の出力ヘルパーは compiler の一括検証・確定とは異なります。
[APIリファレンス](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/API.md)で契約を確認してください。

## 選び方

公開前の生成・検証をまとめたい upload/build 処理が主対象です。幅広い編集や形式が必要なら sharp など、
HTTP変換・保存・配信の運用が主目的なら imgproxy やマネージドサービスとの役割を比較します。
[製品方針](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/PROJECT_PHILOSOPHY.md)を参照してください。

メタデータ除去、Rust、複数サイズ生成だけを独自性とはしません。過去の JPEG サイズ改善は記録された条件に限られ、
現在の全面的な速度・容量優位を意味しません。[性能の読み方](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/PERFORMANCE.md)に根拠をまとめています。

## 安全性と制約

- 入力は JPEG/PNG/WebP、エンジン出力は JPEG/PNG/WebP/AVIF。compiler の出力は JPEG/WebP/AVIF。
- メタデータは既定で除去。`public-upload` は EXIF/GPS/XMP を除去し、検証済み ICC のみ保持します。
- 入力の V8 コピーを避けても native memory は必要です。大きな mmap 入力は処理中に変更・切り詰め・削除しないでください。
- 回転は90°/180°/270°。16-bit 入力は8-bitへ変換し、アニメーション編集・描画は対象外です。

[互換性と制限](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/COMPATIBILITY.md) · [metadata契約](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/METADATA_SUPPORT.md)

## Browser / Edge

[別の Wasm パッケージ](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/WASM_PACKAGE_API.md)は upload preflight 用です。
Node compiler と同じAPIではありません。対象ランタイムで bundle size と latency を確認してください。

## 開発

[開発者向けドキュメント](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/DEVELOPMENT.md)に build、検証、内部仕様、リリース手順があります。

## ライセンス

MIT。使用ライブラリは [英語版 README](./README.md#license) を参照してください。
