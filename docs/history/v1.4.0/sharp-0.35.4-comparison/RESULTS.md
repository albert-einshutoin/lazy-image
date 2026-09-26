# sharp 0.35.4 による保存済み Wasm 品質証拠の比較

解析日時: 2026-09-26T07:38:32.229Z / 解析コードSHA: `6f28daf9e01524d44a325b88c004dddd2fa9de56`。
旧記録: [quality-results.json](../quality-evaluation/quality-results.json)（SHA-256 `2e7f7d67505a27db52bfd3cfd1d08a04129b7298503ad8d6356b8969c9b14c79`、当時の解析コード `15623e001ba0d7b01a448cc9793e968749f4fa88`）。
今回の [baseline.json](baseline.json) と [comparison.json](comparison.json) は別ディレクトリに保存。旧参照・raw・出力・結果は変更していない。

旧: sharp 0.35.0 / libvips 8.18.3 / libheif 1.23.0。
新: sharp 0.35.4 / libvips 8.18.6 / libheif 1.23.2。
採点: ssim.js 3.5.0、元記録と同一のsRGB 8-bit RGB PSNR・SSIM条件。入力、旧参照、保存済み出力はSHA-256検証済み。

## 旧参照・保存済み出力のdecodeと採点

16ケース中、出力画素hashが変わったもの: 0。
旧参照画素hashが変わったもの: 0。
旧参照を固定したSSIM差の最大絶対値: 0、PSNR差: 0 dB。

## 元入力からの参照再生成

| 入力 | PNG bytes hash変化 | デコード後の参照画素変化 | 変更チャネル数 / 全チャネル | 最大絶対差 |
|---|---|---|---:|---:|
| jpeg-webp | false | false | 0 / 10240000 | 0 |
| png-jpeg | false | false | 0 / 10240000 | 0 |
| metadata-budget | false | false | 0 / 172800 | 0 |
| coffee | false | false | 0 / 272640 | 0 |
| chelsea | false | false | 0 / 272640 | 0 |

新参照で採点したSSIM差の最大絶対値: 0、PSNR差: 0 dB。
ケース別のhash、画素digest、両軸のSSIM/PSNR値と差はcomparison.jsonに記録。

保存済みWasm出力bytesが同一なので、この再解析差はWasmの画質回帰を示さない。用途別の画質十分性は対象用途・表示条件・実容量・権利・判定者が揃うまで未判定。
