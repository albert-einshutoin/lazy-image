# sharp 0.35.0→0.35.4 の品質比較手順

旧評価の [固定手順](../quality-evaluation/PROTOCOL.md) と
[結果](../quality-evaluation/quality-results.json)を入力とする。
保存済み16出力（合成4・CC0写真12）、元入力5件、lossless参照5件の
SHA-256を読み込み時に検証する。旧参照・raw evidence・画像・結果は更新しない。
比較専用スクリプトは新ディレクトリだけへ書き込み、解析コードは
`6f28daf9e01524d44a325b88c004dddd2fa9de56`で固定した。

## 実行した順序

macOS arm64、Node v24.2.0で、旧mainの通常install済み
`sharp@0.35.0`が残る作業環境を、#863 merge後main
`0d8a4c04407afd05bd77ed80a62bc927925c136d`から
#844の解析コードcommit `6f28daf9e01524d44a325b88c004dddd2fa9de56`へ
切り替えた。旧版が入っていることを`npm ls sharp`と
`require('sharp').versions.sharp`で確認後、次を実行した。

```sh
node test/benchmarks/wasm-quality-sharp-comparison.mjs baseline
npm ci
node test/benchmarks/wasm-quality-sharp-comparison.mjs updated
```

`npm ci`はmanifest・lockfileを変更せずに既存の`node_modules`を
作り直す通常インストール。新版で`sharp@0.35.4`、
`@img/sharp-darwin-arm64@0.35.4`、
`@img/sharp-libvips-darwin-arm64@1.3.3`を解決した。
`sharp.versions`の実値は[baseline.json](baseline.json)と
[comparison.json](comparison.json)に全項目を保存した。

新しい作業環境から旧版baselineを再現する場合は、同じmacOS arm64と
Node 24系で、まず上記main commitへcheckoutして`npm ci`する。
その`node_modules`を保ったまま解析コードcommitへ切り替えて
`baseline`を実行し、その後上記の`npm ci`と`updated`を実行する。
旧版baseline直前に新版lockfileの`npm ci`を行うと、
旧sharpが失われるため順序を守る。両phaseは解析スクリプト・metrics
helperの未commit差分と解析コードSHAの不一致を拒否する。

## 比較軸

1. **decode・採点:** 当時の参照PNGと保存済み出力を固定し、
   旧・新sharpでsRGB 8-bit RGBAへdecodeした画素SHA-256と、
   同じSSIM／RGB PSNR条件の採点値を比較する。
2. **参照生成:** 当時の元入力と寸法・EXIF autoOrient・sRGB変換・
   inside/Lanczos3・拡大なし・PNGの条件を固定し、新sharpで
   `reference-*.png`を別ディレクトリに生成する。PNGファイルSHA-256と
   デコード後の画素SHA-256・チャネル差を別々に比較し、
   新参照で採点した差も記録する。

同一の保存済みWasm出力について、再解析の差をWasm画質回帰とは扱わない。
この比較はsharp更新の影響確認であり、用途別の画質十分性判定ではない。
