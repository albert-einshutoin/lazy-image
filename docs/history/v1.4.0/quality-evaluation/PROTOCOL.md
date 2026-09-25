# 公開 Wasm v1.4.0 品質評価の固定手順

この手順と測定コードを画像生成前に確定する。開始時の `origin/main` は
`ec5df45de23f4ebe9ca0a3a76bd838190ad34d92`。PR #861 以降の関連変更はない。
既存の raw evidence と画像を変更せず、結果はこのディレクトリへ保存する。

## 入力と容量条件

- 保存済みの Node、Chrome Worker 通常ロード、local workerd の4画像を照合する。
  入力は `test_3.2MB_5000x5000.jpg`、`test_4.5MB_5000x5000.png`、
  保存済み `wasm-metadata-input.jpg`。対応する元の policy、target、bytes、hash を
  `positive-evidence.json` と実ファイルの両方で確認する。同一bytesなら採点は共有し、
  runtime別の元記録と生成日時・コードSHAを結果に残す。
- 追加写真は `release-manifest.json` の CC0 `coffee` と `chelsea` の2件のみ。
  同manifestの `additionalLicenses` にあるCC0原文をhash検証し、path・hashを
  公開npm実行記録とケース別結果に保存する。
  公開 npm `@alberteinshutoin/lazy-image-wasm@1.4.0` をcheckout外に通常installし、
  `/worker` をChrome Dedicated Workerで `wasmModules` なしで実行する。
  出力は JPEG、WebP、resizeは `maxWidth/maxHeight: 320/320, fit: inside`。
  `profile: upload-safe`、公開既定の `minQuality: 45, maxQuality: 85`、
  `qualityFloorPolicy: best-effort` を全条件に固定する。
- 各入力・形式のbudgetなし出力実bytesを `B` とし、追加targetは
  `floor(B * 80 / 100)` と `floor(B * 50 / 100)`。baseline結果を得た直後に
  決定し、変更しない。80/50%は実験条件で、品質の合格閾値ではない。
  budget不達でも有効画像なら採点し、`budgetMet:false` を保持する。

## 共通の lossless 参照

元入力を sharp 0.35.0 / libvips 8.18.3 でデコードする。EXIF Orientationを
`autoOrient()` で適用し、ICCがあれば入力プロファイルからsRGBへ変換し、
なければ入力をsRGBと解釈する。`toColourspace('srgb')` の結果を
メタデータなしのPNGとして一度materializeし、そのSHA-256を記録する。
この中間画像を `resize({width, height, fit:'inside', kernel:'lanczos3',
withoutEnlargement:true, fastShrinkOnLoad:false})` し、メタデータなしのPNGへ出力する。
保存済み入力には元policyの最大寸法、写真には320×320を使う。
PNGは非可逆再符号化せず参照として保存し、入力・参照のSHA-256、
EXIF/ICC/alpha、寸法を記録する。alphaが実画素に存在する組は
不透明画像のRGB指標から除外し、別途合成背景を定義するまで未測定とする。

出力はsharpでデコードしてsRGBの8-bit RGBAへ変換する。出力に含まれる
ICCがあればそのプロファイルから変換する。出力にICCがなければsRGBと
解釈する。出力は採点前にresize・回転・ICC再付与しない。
参照と寸法が違えば理由を保存して未測定にする。

## 指標と判定

不透明画素を確認後、PSNRはsRGB 8-bit RGBの全3チャネルのMSEから
`10 log10(255² / MSE)` とする。完全一致はJSONで
`{"status":"perfect","value":"Infinity"}`、未測定は
`{"status":"unmeasured","value":null,"reason":...}`、計算失敗は
`{"status":"error","value":null,"reason":...}` と区別する。
既存native helperのRGBA PSNRは変更しない。

SSIMは `ssim.js` 3.5.0 の `mssim`。既存helperと同じwindow 8、
実装既定のWeber法、integer RGB→gray、`k1=.01, k2=.03`、8 bit、
original downsample・maxSize 256を明示指定する。内部縮小factorは
ライブラリ実装の `round(min(width,height)/256)`（2以上なら実施）を各行に記録する。
負値は保持する。非有限値や浮動小数点許容誤差を超える1超は計算異常とし、
品質不良そのものは計測失敗としない。

両指標は固定参照と各出力の処理全体の差であり、codecだけの誤差ではない。
budgetなしの非可逆出力を参照には使わない。合格閾値は設けない。
同じ入力・形式のbudgetなしに対するbytes、SSIM、PSNRの差だけ補助的に示す。
JPEG/WebPのquality数値を形式間で同じ知覚品質として比較しない。

参照: [SSIM原典・推奨scale](https://ece.uwaterloo.ca/~z70wang/research/ssim/)、
[ssim.js](https://github.com/obartra/ssim)、[sharp resize](https://sharp.pixelplumbing.com/api-resize/)、
[sharp autoOrient](https://sharp.pixelplumbing.com/api-operation/#autoorient)。
