# 公開Wasm 1.3.1: Chrome Workerの通常ロード

**PASS。** 公開npmの`@alberteinshutoin/lazy-image-wasm@1.3.1`をcheckout外の空ディレクトリへ通常installし、公開`/worker` helperをChromeの実`DedicatedWorkerGlobalScope`で実行した。`wasmModules`、手動codec初期化、`wasmBinary`、fetch差替え、Node用ImageData shimは使っていない。着手時mainは`3db2cf0895a88abdfebb1566bb9ab26d7daca72b`（#854 merge）。測定コードは`5f08b67f76a9594585138fc2ad53ffacd935d499`、測定時dirty=false、対象source filesの内容SHA-256は`92371ed73e931d2f8f3ae86566af4676b269ba0e36336e221e7621b0633e16e0`。2026-09-24 16:50:46 UTCに測定した。

macOS 27.0 arm64、Node v24.2.0、npm 11.3.0、Google Chrome 153.0.8010.53、esbuild 0.25.10。npm registryは`https://registry.npmjs.org/`。公開本体tarballのintegrityは`sha512-80yry+i323threLO1+cl4BkxBIknDBWPgRiQ08Jt3Z98cUfJNZV5WKKCrsfwpcd8fCKC7EhuaDkXrLAxXsxcOw==`。公開codecは`@jsquash/jpeg@1.6.0`、`png@3.1.1`、`resize@2.1.1`、`webp@1.5.0`。各packageの取得URL・integrity、manifestとLICENSEのhash、実esbuild binaryのhashは[raw evidence](./wasm-1.3.1/browser-default/wasm-published-evidence.json)に保存した。bundle inputsは一時install内の公開`worker.js`・`browser.js`と依存codecであり、checkoutの製品コードは混入していない。

再実行はリポジトリrootで次のコマンドを使う。runnerは一時installに公開packageとesbuildを導入し、公開codecから見つけたWasm 9ファイルを**未改変で**`worker.js`と同じHTTP URL階層へcopyする。出力は`artifacts/benchmark/`（gitignored）へ保存される。本記録では[実行ログ](./wasm-1.3.1/browser-default/wasm-published-run.log)、[summary JSON](./wasm-1.3.1/browser-default/wasm-upload-summary.json)、[Markdown](./wasm-1.3.1/browser-default/wasm-upload-summary.md)、[実画像](./wasm-1.3.1/browser-default/)を同じrunから保存した。JSONの`artifacts/benchmark/`は実行時の位置を示し、このディレクトリに同名の保存コピーがある。

```bash
node test/benchmarks/wasm-upload-comparison.bench.js --runtime browser --browser-load default --version 1.3.1
node test/benchmarks/wasm-upload-comparison.bench.js --runtime browser --browser-load default --version 1.3.1 --withhold-wasm mozjpeg_dec.wasm
```

2行目は意図した負例で、非zero終了が期待値。実行順は任意だが、後のrunが`artifacts/benchmark/wasm-published-evidence.json`を上書きするので、各runのJSONとログを別々に保存する。正例は公開`/worker`からesbuildのESM `worker.js`を作り、ページの`main.js`からmodule Workerを起動する。HTTPはJSに`text/javascript`、Wasmに`application/wasm`を返し、全応答に`Cache-Control: no-store`を付ける。公開codec自身が`worker.js`相対URLからWasmを取得した。実要求のURL、HTTP 200/404、Content-Type、転送encoding、asset SHA-256と公開codec内の元ファイルとの対応はraw evidenceの`browserResults.requests`・`requestedWasm`にある。

最初の小JPEG probeは画像処理・codec初期化を先行させず、新規WorkerでJPEG→WebPを実行した。Chromeは`/mozjpeg_dec.wasm`、`/squoosh_resize_bg.wasm`、**SIMD版**`/webp_enc_simd.wasm`を取得。PNG→JPEGで`/squoosh_png_bg.wasm`と`/mozjpeg_enc.wasm`が加わった。実際に要求された5 Wasmを必要配布量へ計上した。明示注入の過去runが用いた非SIMD `webp_enc.wasm`は今回の必要assetに含めない。9ファイルのcopyはこのbundleで他のcodec経路・variantも解決するための配置であり、今回5ファイル以外を実行した証拠ではない。追加JS chunkは出ていない。

| 実際に必要だったasset | raw B | gzip B | SHA-256 |
|---|---:|---:|---|
| `main.js` | 3,368 | 1,329 | `4c8e54ba9d544b38445d372fbdb09bc5f3ee8ab1639d41bc6c138a0fc1e22ef5` |
| `worker.js` | 325,980 | 68,452 | `da8d469e7ed4d8d929964b89b2eca46edcd06c4912c1f0632f6e2e9455482fab` |
| `mozjpeg_dec.wasm` | 166,470 | 63,350 | `a7c4b12169817e779ff4af137981393ae924944e167ad1bd95747c9199162d3e` |
| `squoosh_resize_bg.wasm` | 34,545 | 16,893 | `5b1f702d502c4d0a70b99f78691bd554d566ba95859e4c57af5435955a1d74a5` |
| `webp_enc_simd.wasm` | 345,584 | 125,956 | `39c279269ec1163b987b6d69749458e3d5b03b9585f58b6ca5455b76b504a305` |
| `squoosh_png_bg.wasm` | 181,088 | 84,183 | `263d6e658808a74b72a1a99c5cc1d619237e70c150db6e41d5d84d3d117ab9be` |
| `mozjpeg_enc.wasm` | 251,524 | 59,051 | `24d4177f1c4963e2058b107189249651c61fdef125570e79b1dfb63c8bb49326` |
| **必要ファイル合計** | **1,308,559** | **419,214** | — |

raw/gzipはファイルサイズ。gzip HTTP応答の**body実転送量**は最初の小probeが275,980 B、4つのfresh Workerを使ったrun全体が986,956 Bで、入力画像・HTML・HTTP headerを含まない。Chrome profileはrunごとに新規、HTTP cacheは`no-store`。各ケースに新規Workerを用意し、同じWorkerで2回warm処理した。最初の小probeだけがrun内でcodec処理前のWorkerであり、後続の大画像のcold値は新規Workerだが同じChrome process内の観測である。

入力hashとpolicyは[raw evidenceのfixtures・browserProbeInput](./wasm-1.3.1/browser-default/wasm-published-evidence.json)に記録した。小probe入力は`test_100KB_1057x1057.jpg`（SHA-256 `6dd5004e9cecaaff5a9b315410b0762c4b27b495379f198bfbbfa496464b6f90`）。主要入力は5000×5000 JPEG（`fd44a839bbcf7b58ec257d970e7027bc15ba4ded1f47f2acc4629d5d0627ea96`）とPNG（`1e007789d755ec4a14753e350284950b6f227a467143cd433bb2c993d20174a5`）。metadata専用の[実入力](./wasm-1.3.1/browser-default/wasm-metadata-input.jpg)は`29a8b5ce307f40c8fa546da2e3773341451bcfc60106eb62fb71d8a101dfce70`で、処理前にEXIF/GPS/XMP/ICCの4項目を構造解析で確認した。

| ケース | Worker作成直前→初回結果 / 同一Worker warm中央値 ms | 独立decode・返却値突合後の出力 | 判定 |
|---|---:|---|---|
| 小JPEG→WebP probe | 261.8 / 76.3 | [WebP 320×320、17,484 B](./wasm-1.3.1/browser-default/wasm-browser-default-probe-jpeg-webp.webp) | PASS |
| JPEG→WebP、resize | 2,957.1 / 1,801.8 | [WebP 1600×1600、328,690 B](./wasm-1.3.1/browser-default/wasm-browser-default-jpeg-webp.webp)、500,000 B以下 | PASS |
| PNG→JPEG、resize | 2,713.8 / 1,551.6 | [JPEG 1600×1600、430,035 B](./wasm-1.3.1/browser-default/wasm-browser-default-png-jpeg.jpg)、450,000 B以下 | PASS |
| metadata→JPEG | 75.7 / 32.3 | [JPEG 180×240、5,886 B](./wasm-1.3.1/browser-default/wasm-browser-default-metadata-budget.jpg)、30,000 B以下 | PASS |

画像はWorkerから実bytesを受け、外側のsharpで形式・寸法・サイズを独立確認し、返却値と照合した。画像生成**4/4**、達成可能なbudget **3/3**。metadata専用入力にあったEXIF/GPS/XMP/ICCに対し、出力のEXIF/XMP/ICCが無いことを確認した。GPSはEXIF内なのでEXIF除去により残らない。通常のJPEG/PNG入力に4項目は元々無く、そのsummary行の`metadataStripped`は`null`で、除去を実証したとは扱わない。10 Bの達成不能budgetでは`best-effort`が[有効なJPEG 3,078 B](./wasm-1.3.1/browser-default/wasm-browser-default-metadata-budget-best-effort.jpg)と`budgetMet:false`を返し、budget未達として集計。`strict`は`E502`で**1/1期待拒否**。期待拒否を画像生成やbudget達成へ加算していない。各出力のhash・品質設定・metricsはraw evidenceにある。

時間は呼び出し側Chrome `performance.now()`で測った1回の初回と2回のwarmの中央値で、比較優位や閾値判定ではない。内部`totalMs`はoptimizer作成やWorker起動を含まない。`instantiateMs`は公開APIの`initializeCodecs`区間だけで、通常ロードで実際に発生したWasm fetch・codec初期化全体ではない。ピークmemoryとcodec初期化単独の時間は取得していない。ブラウザ以外の性能へ一般化しない。

[欠落負例のraw evidence](./wasm-1.3.1/browser-default/missing-wasm-evidence.json)と[非zeroログ](./wasm-1.3.1/browser-default/missing-wasm-run.log)は同じ測定コード`5f08b67…`、dirty=false、2026-09-24の別run。`mozjpeg_dec.wasm`を配信せずfresh Workerで開始すると、同URLがHTTP 404、最初の小JPEG変換が`E131`で失敗し、runnerはexit 1・verdict FAILとなった。PNGケースが成功してもrun全体をPASSにしない。現行codecは404応答本文のWasm instantiate失敗をJPEG decode errorとして返すため、ヒント文は入力破損を示すが、真因は配信asset欠落である。公開1.3.1の通常ロード自体は上記構成でPASS。既存の[明示注入Chrome実測](./WASM_1.3.1_VERIFICATION.md)と[local workerd実測](./WASM_1.3.1_EDGE_VERIFICATION.md)は元の日時・測定SHAのまま別証拠として保持する。
