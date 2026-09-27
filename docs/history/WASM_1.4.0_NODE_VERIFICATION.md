# 公開Wasm v1.4.0のNode実測

**判定: 固定したNodeの1構成でPASS。** 画像生成は通常3変換と10 B best-effortの**4/4**、達成可能budgetは通常3変換の**3/3**、strictのE502期待拒否は別に**1/1**。破損JPEG E131とresize初期化 E503は、それぞれ別Nodeプロセスで画像処理FAIL／期待診断PASSとした。対象は公開npmからcheckout外へ通常installした`@alberteinshutoin/lazy-image-wasm@1.4.0`であり、checkoutの製品コードを実行した結果ではない。[v1.3.1のNode実測](WASM_1.3.1_VERIFICATION.md)と[v1.4.0のChrome・native・local workerd実測](V1.4.0_VERIFICATION.md)は日時・対象version・runtimeが異なる別証拠として保持する。

着手時mainは`5defd7ea3228592209996b2a16b98f94563be666`、画像処理を実行した測定コードは`4c356dcd7fc90a7917dfa378f2480e07ccab0356`（dirty=false、対象source files SHA-256 `f6c87568e850100ed181e68e1bab8a299825495e5661227cdaed097b39313404`）。正例は2026-09-25 12:56:25 UTC、負例は12:57:10と12:57:21 UTC。保存した[正例raw evidence](v1.4.0/node-wasm/positive-evidence.json)と[summary JSON](v1.4.0/node-wasm/wasm-upload-summary.json)／[Markdown](v1.4.0/node-wasm/wasm-upload-summary.md)には実測値、入力hash、policy、出力hashを含む。検証記録へ後から行う再解析は、この実測日時・commitでの新規画像処理と混同しない。

## 環境・公開物

- macOS **27.0** arm64（Darwin release 27.0.0）、Node **v24.2.0**、npm **11.3.0**。3 runのNode PIDは正例98052、破損JPEG 99656、resize初期化289。各runが新しいプロセスと空の一時install先を使用した。
- registryは`https://registry.npmjs.org/`。本体manifestとインストール済み`shared.js`の`VERSION`は両方1.4.0。公開tarballは`https://registry.npmjs.org/@alberteinshutoin/lazy-image-wasm/-/lazy-image-wasm-1.4.0.tgz`、integrityは`sha512-R3xpMJr3rBv+LsyfpggixCZvktlS3XRvLi542x4JgbrvvBP3NH/xip3TJSze4fGDQIfj7hWplLeKUlHPCnidew==`。
- 公開codecは`@jsquash/jpeg@1.6.0`、`png@3.1.1`、`resize@2.1.1`、`webp@1.5.0`。各packageと同時installしたesbuild 0.25.10の解決URL・integrity・license hashはraw evidenceの`packages`を参照。Node正例でesbuildは画像処理には使わない。
- 実importは隔離install先の`node_modules/@alberteinshutoin/lazy-image-wasm/browser.js`をfile URLで指定した。インストール済みmanifestの公開`./browser` exportが`./browser.js`を指すこともassertした。`ImageData`だけをNodeでshimし、公開codec由来の5つのWasm **bytes**を`wasmModules`へ明示注入した。APIの`metrics.runtime: browser`は実行場所を表さず、結果名は**「Node上のWasm実行／ImageData shim・Wasm bytes明示注入」**とする。

| 注入キー | 公開codecのWasm | SHA-256 |
|---|---|---|
| `jpegDecode` | `@jsquash/jpeg/codec/dec/mozjpeg_dec.wasm` | `a7c4b12169817e779ff4af137981393ae924944e167ad1bd95747c9199162d3e` |
| `jpegEncode` | `@jsquash/jpeg/codec/enc/mozjpeg_enc.wasm` | `24d4177f1c4963e2058b107189249651c61fdef125570e79b1dfb63c8bb49326` |
| `pngDecode` | `@jsquash/png/codec/pkg/squoosh_png_bg.wasm` | `263d6e658808a74b72a1a99c5cc1d619237e70c150db6e41d5d84d3d117ab9be` |
| `resize` | `@jsquash/resize/lib/resize/pkg/squoosh_resize_bg.wasm` | `5b1f702d502c4d0a70b99f78691bd554d566ba95859e4c57af5435955a1d74a5` |
| `webpEncode` | `@jsquash/webp/codec/enc/webp_enc.wasm` | `b6085bb6702f144e9dc6016d58d230b34a84976bf0d080b7390b4b4b137d6ab7` |

NodeはブラウザのSIMD版自動選択やHTTP取得を測っていない。上表は実際に注入した5件である。

## 画像・policy・時間

入力と設定は正例raw evidenceの`fixtures`に記録した。JPEG入力SHA-256は`fd44a839bbcf7b58ec257d970e7027bc15ba4ded1f47f2acc4629d5d0627ea96`、PNGは`1e007789d755ec4a14753e350284950b6f227a467143cd433bb2c993d20174a5`、[metadata専用入力](v1.4.0/node-wasm/wasm-metadata-input.jpg)は`29a8b5ce307f40c8fa546da2e3773341451bcfc60106eb62fb71d8a101dfce70`。最後の入力にEXIF・GPS tag・XMP・ICCが存在することを先に確認した。

| ケース | 保存出力と独立decode結果 | target | 初回／warm中央値 |
|---|---|---:|---:|
| JPEG→WebP、1600×1600 | [328,690 B WebP](v1.4.0/node-wasm/wasm-node-jpeg-webp.webp)、SHA-256 `cc80ef7b1bc8488ba195f9ad50e17af75c46dac8fdbb8f13f8f5b06644462eec` | 500,000 B達成 | 3,846.7／2,688.5 ms |
| PNG→JPEG、1600×1600 | [430,035 B JPEG](v1.4.0/node-wasm/wasm-node-png-jpeg.jpg)、SHA-256 `301fd3e0b8cbfb2d8c6b004efb54243fa7f5d9186a3f0bab569c5ff26d58b67a` | 450,000 B達成 | 2,389.6／2,346.9 ms |
| metadata専用JPEG、180×240 | [5,886 B JPEG](v1.4.0/node-wasm/wasm-node-metadata-budget.jpg)、SHA-256 `3d4de1967d2db5ff428ef4ce895aacc5d5c35e8c0d271043bdcb44f626b12ba0` | 30,000 B達成 | 41.2／38.9 ms |
| 同入力、10 B best-effort | [3,078 Bの有効JPEG](v1.4.0/node-wasm/wasm-node-metadata-budget-best-effort.jpg)、`budgetMet:false` | **不達** | 9.0 ms、warm対象外 |

同じ10 B条件のstrictは画像を返さずE502で期待どおり拒否した。sharpで保存bytesを独立にデコードし、形式・寸法・実bytes・SHA-256を結果と照合した。metadata専用出力からEXIF・XMP・ICCがなくなり、GPSを含むEXIFも残らないことを確認した。metadataがない通常JPEG/PNG入力の除去実証は`null`である。

`optimizerCreationMs`はimportとWasmファイル読み込み**後**のoptimizer作成時間で、正例は9.4 ms。各ケースの初回値は作成後最初の`optimizeUpload()`、warmは同じoptimizerで続けて行った2回の中央値であり、optimizerを3ケース間でも共有した。内部`firstEncodeMs`はbudget探索の最初のencode試行だけで、全処理時間ではない。Node行のbrowser／Edge配信量はJSONで`null`、Markdownで`n/a`。optionalなlocal native参照行はcheckoutのnative bindingによる別結果で、公開Wasmの結果に含めない。性能合格閾値やブラウザ・Edgeの速度主張は設定しない。

## 公開APIの診断

| 独立Node run | 意図した失敗・実際の返却値 | 保存証拠 |
|---|---|---|
| JPEGシグネチャ`ff d8 ff 00 00 00`を持つ[破損入力](v1.4.0/node-wasm/wasm-node-corrupt-jpeg-input.jpg) | optimizer作成後の画像処理でFAIL。E131／`CodecError`／`recoverable:false`。`message`: `jpeg decoder failed during codec initialization or image decoding: Program terminated with exit(1)` | [raw](v1.4.0/node-wasm/corrupt-jpeg-evidence.json)、[log](v1.4.0/node-wasm/corrupt-jpeg-run.log) |
| `resize`だけを`new Uint8Array([0])`へ変更。[入力](v1.4.0/node-wasm/wasm-node-resize-init-input.jpg)は正常JPEG | optimizer初期化でFAIL。E503／`CodecError`／`recoverable:false`。`message`: `resize codec initialization failed: WebAssembly.compile(): expected 4 bytes, fell off end @+0` | [raw](v1.4.0/node-wasm/resize-init-evidence.json)、[log](v1.4.0/node-wasm/resize-init-run.log) |

両runの`recoveryHint`全文は公開APIの返却値のままraw evidenceの`nodeDiagnostic.apiError`へ保存した。破損画像のhintはWasm配置・DevTools Networkを確認し、配信が正常なら入力破損・codec処理を確認するよう案内する。resizeのhintは対応Wasmと`wasmModules`の対応を確認するよう案内する。Nodeで実際のWasm HTTP request・statusを観測したわけではない。resize負例の1 byte入力SHA-256は`6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d`で、他4 moduleは正例と同じ公開bytes。各runの`nodeStage`は測定側の到達段階、`apiError`はAPIから受けた値であり、欠損値を推定していない。負例は画像処理FAILなのでCLI exit 1を維持し、別の`diagnosticValidation.status: PASS`で期待診断を確認した。

## 再実行と保存

リポジトリrootで以下を**順に**実行する。各コマンドは新しいNodeプロセス・新しいcheckout外一時installを作る。`artifacts/benchmark/`内のraw evidence、入力・出力画像、summaryは次のrunで上書きされうるため、各run直後に別名でコピーする。以下は新しい一時ディレクトリへ保存する再現例であり、今回の確定結果は上記リンクの`docs/history/v1.4.0/node-wasm/`へ保存した。負例はexit 1が期待値だが、`diagnosticValidation.status: PASS`、code・stageを確認するまでは検証PASSにしない。

```bash
set -e
evidence_dir=$(mktemp -d)
node test/benchmarks/wasm-upload-comparison.bench.js --runtime node --version 1.4.0 > "$evidence_dir/positive-run.log" 2>&1
cp artifacts/benchmark/wasm-published-evidence.json "$evidence_dir/positive-evidence.json"
cp artifacts/benchmark/wasm-upload-summary.json artifacts/benchmark/wasm-upload-summary.md "$evidence_dir/"
cp artifacts/benchmark/wasm-metadata-input.jpg artifacts/benchmark/wasm-node-jpeg-webp.webp artifacts/benchmark/wasm-node-png-jpeg.jpg artifacts/benchmark/wasm-node-metadata-budget.jpg artifacts/benchmark/wasm-node-metadata-budget-best-effort.jpg "$evidence_dir/"
if node test/benchmarks/wasm-upload-comparison.bench.js --runtime node --version 1.4.0 --node-diagnostic corrupt-jpeg > "$evidence_dir/corrupt-jpeg-run.log" 2>&1; then exit 1; fi
node -e 'const e=require("./artifacts/benchmark/wasm-published-evidence.json");if(e.diagnosticValidation?.status!=="PASS"||e.nodeDiagnostic?.apiError?.code!=="E131"||e.nodeDiagnostic?.failureStage!=="image-processing")process.exit(1)'
cp artifacts/benchmark/wasm-published-evidence.json "$evidence_dir/corrupt-jpeg-evidence.json"
cp artifacts/benchmark/wasm-node-corrupt-jpeg-input.jpg "$evidence_dir/"
if node test/benchmarks/wasm-upload-comparison.bench.js --runtime node --version 1.4.0 --node-diagnostic resize-init > "$evidence_dir/resize-init-run.log" 2>&1; then exit 1; fi
node -e 'const e=require("./artifacts/benchmark/wasm-published-evidence.json");if(e.diagnosticValidation?.status!=="PASS"||e.nodeDiagnostic?.apiError?.code!=="E503"||e.nodeDiagnostic?.failureStage!=="optimizer-initialization")process.exit(1)'
cp artifacts/benchmark/wasm-published-evidence.json "$evidence_dir/resize-init-evidence.json"
cp artifacts/benchmark/wasm-node-resize-init-input.jpg "$evidence_dir/"
echo "$evidence_dir"
node test/integration/wasm-node-1.4.0-evidence.test.js
```

runの保存名は`positive-evidence.json`／`positive-run.log`、`corrupt-jpeg-evidence.json`／`corrupt-jpeg-run.log`、`resize-init-evidence.json`／`resize-init-run.log`で、正例summaryと各入力・出力画像を同じディレクトリへ保存した。最後の保存済み証拠テストは画像処理を再測定せず、上記測定コード`4c356dcd…`が生成したcommitted bytes・JSONを検査する。

今回の確認はmacOS 27.0 arm64／Node v24.2.0の1構成、`ImageData` shimとWasm bytes明示注入に限る。Node.js 22・他OS・他version、ブラウザ通常ロード、Cloudflare本番は今回のNode再検証には含めない。後者の既存検証範囲はそれぞれの記録を参照する。
