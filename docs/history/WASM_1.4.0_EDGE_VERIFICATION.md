# 公開Wasm 1.4.0: local workerd再検証

**選定したlocal workerd構成でPASS。** 着手時mainは`a517f14a16acbe8ff380def23c4259ceeeae5485`、実測コードは`66aa6ff8a50759fa494838d2ad8a32a7ef97fa14`（dirty=false、対象source files SHA-256 `f00002caa2a013195f960455e9f1e4cffa73c2507d358a3ecc1174c9193c778e`）。正例は2026-09-25 10:47:10 UTC、4負例は同日10:47:30–36 UTCの別runである。測定対象は公開npmの`@alberteinshutoin/lazy-image-wasm@1.4.0`。 [実installのversion確認](v1.4.0/edge-workerd/version-check.json)でpackage manifestと`shared.js`の`VERSION`がともに1.4.0、resolvedが公開registry、integrityが`sha512-R3xpMJr3rBv+LsyfpggixCZvktlS3XRvLi542x4JgbrvvBP3NH/xip3TJSze4fGDQIfj7hWplLeKUlHPCnidew==`であることを確認した。[v1.3.1のEdge実測](WASM_1.3.1_EDGE_VERIFICATION.md)と[v1.4.0のnative・Chrome公開後実測](V1.4.0_VERIFICATION.md)は各々元の日時・versionの証拠として保持する。

macOS 27.0 arm64、Node v24.2.0、npm 11.3.0、`workerd@1.20260924.1`（実行ファイルは`workerd 2026-09-24`を表示）、esbuild 0.25.10。workerdのcompatibility dateは`2026-09-24`、追加flagsなし。runnerはcheckout外の空の一時ディレクトリに公開package、codec、esbuild、workerdを`https://registry.npmjs.org/`から通常installした。公開本体・`@jsquash/jpeg@1.6.0`・`png@3.1.1`・`resize@2.1.1`・`webp@1.5.0`、実行toolchainとplatform binaryのresolved URL、integrity、manifest/license/binary hash、実bundle入力は[正例raw evidence](v1.4.0/edge-workerd/positive-evidence.json)にある。実workerd binaryのSHA-256は`354615e8d5ccbc2ab9afff5ec7f9a3a6e21f83bb27c314c4c65685d1fbaf984c`で、隔離install内の公開platform packageと一致した。Node用ImageData/DOM shimはEdgeへ追加していない。

公開`/edge`の`createUploadOptimizer()`をbundleし、以下の**公開codecの未改変Wasmを静的`WebAssembly.Module`として正しいキーに注入**した。workerdの`/health`で実Module型5件・optimizer作成0・isolate IDを確認してから画像requestを送った。画像のdecode、resize、encode、budget探索はworkerd isolate内で行い、Node側は起動・外側の時間計測・返却された実bytesの独立検査を担当した。APIの`metrics.runtime`だけで実行場所を判定していない。bundle入力は隔離install内の公開`edge.js`を含み、checkout内のpackageを含まない。

| `wasmModules`キー | 静的module | SHA-256 |
|---|---|---|
| `jpegDecode` | `@jsquash/jpeg/codec/dec/mozjpeg_dec.wasm` | `a7c4b12169817e779ff4af137981393ae924944e167ad1bd95747c9199162d3e` |
| `jpegEncode` | `@jsquash/jpeg/codec/enc/mozjpeg_enc.wasm` | `24d4177f1c4963e2058b107189249651c61fdef125570e79b1dfb63c8bb49326` |
| `pngDecode` | `@jsquash/png/codec/pkg/squoosh_png_bg.wasm` | `263d6e658808a74b72a1a99c5cc1d619237e70c150db6e41d5d84d3d117ab9be` |
| `resize` | `@jsquash/resize/lib/resize/pkg/squoosh_resize_bg.wasm` | `5b1f702d502c4d0a70b99f78691bd554d566ba95859e4c57af5435955a1d74a5` |
| `webpEncode` | `@jsquash/webp/codec/enc/webp_enc.wasm` | `b6085bb6702f144e9dc6016d58d230b34a84976bf0d080b7390b4b4b137d6ab7` |

入力fixtureのSHA-256と全policy（品質上下限を含む）は[raw evidenceの`fixtures`](v1.4.0/edge-workerd/positive-evidence.json)に保存した。小JPEG probeは`6dd5004e9cecaaff5a9b315410b0762c4b27b495379f198bfbbfa496464b6f90`。通常JPEGは`fd44a839bbcf7b58ec257d970e7027bc15ba4ded1f47f2acc4629d5d0627ea96`、PNGは`1e007789d755ec4a14753e350284950b6f227a467143cd433bb2c993d20174a5`。metadata専用の[保存入力](v1.4.0/edge-workerd/wasm-metadata-input.jpg)は`29a8b5ce307f40c8fa546da2e3773341451bcfc60106eb62fb71d8a101dfce70`で、処理前の構造検査でEXIF・GPS・XMP・ICCの存在を確認した。通常2入力にはこれらのmetadataがないため、除去実証に加算しない。

| 正例 | 起動前→初回応答 / 初回request / warm中央値（ms） | 独立decodeした保存画像 | 判定 |
|---|---:|---|---|
| 小JPEG→WebP probe（集計対象外） | 223.9 / 191.0 / 114.6 | [320×320、17,484 B](v1.4.0/edge-workerd/wasm-edge-probe-jpeg-webp.webp) | PASS |
| JPEG→WebP、resize | 3,711.8 / 3,681.4 / 2,600.6 | [1600×1600、328,690 B](v1.4.0/edge-workerd/wasm-edge-jpeg-webp.webp)、500,000 B以下 | PASS |
| PNG→JPEG、resize | 3,114.4 / 3,083.6 / 2,017.6 | [1600×1600、430,035 B](v1.4.0/edge-workerd/wasm-edge-png-jpeg.jpg)、450,000 B以下 | PASS |
| metadata→JPEG | 97.2 / 66.7 / 41.9 | [180×240、5,886 B](v1.4.0/edge-workerd/wasm-edge-metadata-budget.jpg)、30,000 B以下 | PASS |

画像生成**4/4**は通常3変換＋10 Bのbest-effortであり、小probeは別枠。達成可能budget**3/3**は通常3変換だけである。10 Bのbest-effortは[有効なJPEG 3,078 B](v1.4.0/edge-workerd/wasm-edge-metadata-budget-best-effort.jpg)と`budgetMet:false`を返し、budget達成に含めない。同条件のstrictは**1/1期待拒否 E502**（`ResourceLimit`、`recoverable:true`）。strict拒否を画像生成へ加算しない。全返却bytesを外側のsharpで独立decodeし、形式・寸法・実bytes・SHA-256をAPI返却値と突合した。[保存成果物検査](../../test/integration/wasm-edge-1.4.0-evidence.test.js)はコピー後の画像・metadata入力を再解析する。metadata専用出力にはEXIF/XMP/ICCがなく、GPSを含むEXIF自体が除去されている。

4負例は各々**新しいworkerdプロセスとisolate**で行い、元画像処理の`FAIL`と期待診断の`PASS`を別に記録した。`code`・`category`・`recoverable`・`message`・`recoveryHint`は公開APIから得た値を測定Workerがそのまま返し、`phase`・`moduleOverride`・HTTP 500は測定Worker側の情報である。HTTP 500をpackageのHTTP契約やWasm取得statusとして扱わない。静的module注入ではWasmのHTTP取得は発生せず、URL/statusは公開APIから得ていない。

| 負例と証拠 | 変更したmodule / 実際の段階 | 公開APIから返った`message` | 画像 / 診断 |
|---|---|---|---|
| [破損JPEG](v1.4.0/edge-workerd/corrupt-jpeg-evidence.json) ([log](v1.4.0/edge-workerd/corrupt-jpeg-run.log)、[入力](v1.4.0/edge-workerd/wasm-edge-corrupt-jpeg-input.jpg)) | 正しい5 module / image-processingのdecode | `jpeg decoder failed during codec initialization or image decoding: Program terminated with exit(1)` | FAIL / E131 PASS |
| [decoder初期化](v1.4.0/edge-workerd/decoder-init-evidence.json) ([log](v1.4.0/edge-workerd/decoder-init-run.log)) | `jpegDecode`だけを有効な最小Wasm bytes `0061736d01000000`へ変更 / optimizer-initialization | `jpeg decoder codec initialization failed: WebAssembly.compile(): Wasm code generation disallowed by embedder` | FAIL / E131 PASS |
| [resize初期化](v1.4.0/edge-workerd/resize-init-evidence.json) ([log](v1.4.0/edge-workerd/resize-init-run.log)) | `resize`だけを`mozjpeg_dec.wasm`の静的Moduleへ変更 / optimizer-initialization | `resize codec initialization failed: WebAssembly.instantiate(): Import #0 "a": module is not an object or function` | FAIL / E503 PASS |
| [encoder初期化](v1.4.0/edge-workerd/encoder-init-evidence.json) ([log](v1.4.0/edge-workerd/encoder-init-run.log)) | `webpEncode`だけを`mozjpeg_dec.wasm`の静的Moduleへ変更 / optimizer-initialization | `webp encoder codec initialization failed: WebAssembly.Instance(): Import #23 "a" "x": function import requires a callable` | FAIL / E300 PASS |

全4ケースは`CodecError`・`recoverable:false`。`recoveryHint`全文、入力checksum、5件の実効module割当とhash、元例外、isolate IDは各raw evidenceの`edgeResults.diagnostic`に保存した。破損JPEGは`ff d8 ff 00 00 00`でJPEGシグネチャを持ち、形式判定後にdecoderが拒否した。E111の形式不一致とは区別した。decoderは公開`@jsquash/jpeg`の初期化が遅延するため、**誤った静的Moduleの割当だけではoptimizer作成時に拒否されず、decode時にE131となる**。この負例は有効なWasm bytesの動的コンパイルをworkerdが拒否する条件で、公開APIの初期化時E131を確認した。誤った静的decoder Moduleを初期化時に検出する証拠ではない。正例の5 moduleはすべて未改変の静的Moduleである。

[起動前失敗ログ](v1.4.0/edge-workerd/fail-closed.log)では`webp_enc.wasm`の静的ファイルを欠くと設定の`embed`を読めず、`isolate-launch`でFAIL、`apiReached:false`となる。E300などのAPI診断やHTTP 404をこのケースから主張しない。誤った静的decoder Moduleを割り当てたprobeはAPI到達後のE131でFAIL、`apiReached:true`となり、起動前失敗と区別できる。workerd実行ファイル欠落はruntime-resolutionのBLOCKED、別binaryへの差替えは同段階のFAIL、非JPEG文字列の画像失敗はimage-processingのE111 FAILと別に記録した。

呼び出し側Nodeの単調増加時計で、新規workerd起動前から初回画像bytes受信、起動後の最初の画像request、同じisolate・optimizerのwarm 2回を測った。表のcoldは各1観測、warmは2回の中央値。画像処理を先行しない`/health`成功を起動済み条件にした。workerd内`performance.now()`はprobe・PNG・metadataの200万回CPUループで2〜3 ms進んだ。JPEG変換後の追加`/clock` requestは接続終了し、そのケースだけ時計診断を理由付き`unavailable`とした。内部`totalMs`は起動やoptimizer作成を含まない。`instantiateMs`だけをWasm取得・初期化全体の時間としない。メモリは取得していない。

静的配布サイズはEdge JS bundle **328,216 B raw / 69,404 B gzip**と上記5 Wasmの合計 **1,243,104 B raw / 406,832 B gzip**。追加JS chunkなし。各assetの個別raw/gzip/hashは正例raw evidenceの`edgeResults.delivery`にある。これは静的配置に必要なファイルサイズであり、ブラウザのHTTP転送量、package全体のサイズ、本番Cloudflareのcold-startや課金CPUを意味しない。

再実行はrepo rootで以下を使用する。各実行は`artifacts/benchmark/wasm-published-evidence.json`を上書きするため、**各commandの直後**にそのJSON、console log、画像を別名で保存する。負例は意図した画像処理FAILのためCLI exit 1が正しく、`diagnosticValidation.status: PASS`を併せて確認する。保存済みrunの[正例ログ](v1.4.0/edge-workerd/positive-run.log)、[summary JSON](v1.4.0/edge-workerd/wasm-upload-summary.json)・[Markdown](v1.4.0/edge-workerd/wasm-upload-summary.md)は上表と同じ正例runを示す。

```bash
git checkout 66aa6ff8a50759fa494838d2ad8a32a7ef97fa14
node test/benchmarks/wasm-upload-comparison.bench.js --runtime edge --version 1.4.0
node test/benchmarks/wasm-upload-comparison.bench.js --runtime edge --version 1.4.0 --edge-diagnostic corrupt-jpeg
node test/benchmarks/wasm-upload-comparison.bench.js --runtime edge --version 1.4.0 --edge-diagnostic decoder-init
node test/benchmarks/wasm-upload-comparison.bench.js --runtime edge --version 1.4.0 --edge-diagnostic resize-init
node test/benchmarks/wasm-upload-comparison.bench.js --runtime edge --version 1.4.0 --edge-diagnostic encoder-init
node test/benchmarks/wasm-edge-failure-check.mjs 1.4.0
```

保存済み証拠テストは測定後の記録commitで実行する。測定commitだけには今回の新しいraw evidenceがまだ無く、測定コマンドと保存証拠の確認revisionを混同しない。

今回の確認範囲は上記1種類のlocal workerd、静的Module明示注入、代表fixture/policyである。他のEdge runtime、Cloudflare本番の性能、Edgeの暗黙Wasmロードは未検証。公開v1.4.0の診断が返ることと、正しいmodule配置時の画像処理を確認したため、この選定構成の再検証は完了とする。#678はv1.3.1の受入条件でCLOSEDのまま、#703はv1.xトラッカーとしてOPENを維持する。
