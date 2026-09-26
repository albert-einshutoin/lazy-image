# 公開Wasm v1.4.0 品質評価

解析日時: 2026-09-25T19:24:20.261Z / 解析コード: `15623e001ba0d7b01a448cc9793e968749f4fa88`.
方法: [固定手順](PROTOCOL.md)。raw: [写真の公開npm実行](photo-run-evidence.json)、[ケース別JSON](quality-results.json)、[並列画像・同座標crop](visual-comparison.html)、[目視所見](OBSERVATIONS.md)。

測定完了と画質の十分性は別判定。品質の合格閾値は設定していない。SSIM/PSNRは処理全体の出力と固定lossless参照との差。

## 保存済み出力（3 runtimeで同一bytes、採点は4標本）

| ケース | target / bytes | budget | quality | SSIM | PSNR dB |
|---|---:|---|---:|---:|---:|
| jpeg-webp | 500000 / 328690 | true | 86 | 0.99576 | 36.20 |
| png-jpeg | 450000 / 430035 | true | 88 | 0.99658 | 34.73 |
| metadata-budget | 30000 / 5886 | true | 86 | 0.98274 | 34.30 |
| metadata-budget-best-effort | 10 / 3078 | false | 未記録 | 0.94246 | 30.20 |

各runtimeの元実行日時・コードSHA・raw evidence hash・出力画像はJSONの`imageGeneration`に対応付けた。10 B best-effortの選択qualityは当時のrawに未記録。

## 実写真（Chrome Worker通常ロード、12ケース）

| 入力 | 形式 | 条件 | target / bytes | budget | quality | SSIM | PSNR dB | bytes差 | SSIM差 | PSNR差 dB |
|---|---|---|---:|---|---:|---:|---:|---:|---:|---:|
| coffee | jpeg | none | なし / 19317 | true | 85 | 0.95020 | 32.25 | — | — | — |
| coffee | jpeg | 80% | 15453 / 13466 | true | 79 | 0.93634 | 30.97 | -5851 | -0.01386 | -1.28 |
| coffee | jpeg | 50% | 9658 / 9506 | true | 64 | 0.90583 | 29.61 | -9811 | -0.04438 | -2.64 |
| coffee | webp | none | なし / 17008 | true | 85 | 0.97327 | 32.95 | — | — | — |
| coffee | webp | 80% | 13606 / 13380 | true | 79 | 0.96151 | 32.17 | -3628 | -0.01177 | -0.78 |
| coffee | webp | 50% | 8504 / 8492 | true | 51 | 0.93041 | 30.45 | -8516 | -0.04286 | -2.50 |
| chelsea | jpeg | none | なし / 14113 | true | 85 | 0.97126 | 36.91 | — | — | — |
| chelsea | jpeg | 80% | 11290 / 10260 | true | 79 | 0.96148 | 35.47 | -3853 | -0.00978 | -1.44 |
| chelsea | jpeg | 50% | 7056 / 7016 | true | 62 | 0.93638 | 33.51 | -7097 | -0.03487 | -3.39 |
| chelsea | webp | none | なし / 12300 | true | 85 | 0.97920 | 37.27 | — | — | — |
| chelsea | webp | 80% | 9840 / 9626 | true | 79 | 0.96917 | 35.82 | -2674 | -0.01003 | -1.45 |
| chelsea | webp | 50% | 6150 / 6148 | true | 55 | 0.94061 | 33.44 | -6152 | -0.03859 | -3.83 |

bytes差・SSIM差・PSNR差は同じ入力・形式のbudgetなしを基準にした補助値。JPEGとWebPのquality数値を相互比較しない。

## 方法と限界

- 参照: sharp 0.35.0 / libvips 8.18.3、EXIF autoOrient、入力ICC→sRGB、inside/Lanczos3、拡大なし、PNG。入力・参照・出力のhashと寸法はJSON。
- PSNR: 不透明sRGB 8-bit RGB、alphaは誤差平均から除外。既存native向けRGBA helperは変更していない。
- SSIM: ssim.js 3.5.0、Weber、window 8、integer grayscale、8 bit、原実装の内部downsample条件をJSONに記録。負値は保持する。
- decode、ICC処理、resize、codecの差を含む。2写真と固定の合成fixtureから一般的upload画像の品質維持・知覚的同等性は保証しない。
- 目視資料は同一座標・倍率の比較であり主観評価実験ではない。
