# 公開Wasm 1.3.1: Node実測とChrome Web Worker

2026-09-24 11:55:31 UTCの測定。着手時mainは`af9a3ebc2419406f631f4e6946911a8708343c27`。**測定コードは`ed873ffb729b0a6bb2214ceedcc2338d66b34809`（dirty=false、4ファイル内容SHA-256 `72533cdfb122e3f7f8ac8d41d061639354dd6e0a89b3736cc9ee1387ca35f002`）、対象は公開npmの`@alberteinshutoin/lazy-image-wasm@1.3.1`**。Node／Chrome Web WorkerともPASS。Edge isolateは未実行で、#678全体は継続する。

再実行コマンドはリポジトリのrootで`npm run test:bench:wasm -- --version 1.3.1`。実測はmacOS 27.0 arm64、Node v24.2.0、npm 11.3.0、Google Chrome 153.0.8010.53（headless=new）、esbuild 0.25.10。公開registry `https://registry.npmjs.org/`からOS一時ディレクトリへ通常installし、`browser.js`の実import先とbundleの入力がそのインストール先にあることをassertした。一時ディレクトリはrun後に削除する。取得元・全codecのversion/integrity・実import先は[詳細JSON](./wasm-1.3.1/wasm-published-evidence.json)を参照。本体tarball integrityは`sha512-80yry+i323threLO1+cl4BkxBIknDBWPgRiQ08Jt3Z98cUfJNZV5WKKCrsfwpcd8fCKC7EhuaDkXrLAxXsxcOw==`。

[実行ログ](./wasm-1.3.1/wasm-published-run.log)・[raw evidence](./wasm-1.3.1/wasm-published-evidence.json)・[保存画像と元入力](./wasm-1.3.1/)は元のrunで保存した。[訂正済み集計JSON](./wasm-1.3.1/wasm-upload-summary.json)・[Markdown](./wasm-1.3.1/wasm-upload-summary.md)は、そのraw evidenceから後述のとおり再生成した。各入力のSHA-256、policy（寸法、品質範囲、byte-budget、best-effort/strict）、各生成画像のSHA-256と実bytesはraw evidenceにある。JPEG→WebPとPNG→JPEGは既存の5000×5000 fixtureを使用。metadataケースの元画像`metadata-1.jpg`は[release corpus manifest](../../test/benchmarks/corpus/release-manifest.json)記載のプロジェクト生成MIT fixture（SHA-256 `ff7c61bc5ba08c18ce0c57fa49d61dd6af0d5d1264f9dc398715af2d6ab01b6d`）で、EXIF/GPS/ICCを保ちXMPを追加した[検証入力](./wasm-1.3.1/wasm-metadata-input.jpg)のSHA-256は`29a8b5ce307f40c8fa546da2e3773341451bcfc60106eb62fb71d8a101dfce70`。

### PR #853の集計訂正（画像処理の再実測ではない）

画像処理は2026-09-24 11:55:31 UTC、測定コード`ed873ffb729b0a6bb2214ceedcc2338d66b34809`、公開npm v1.3.1で実行した。元の集計は11:56:04 UTCの[訂正前JSON](./wasm-1.3.1/wasm-upload-summary-original.json)として保持した。2026-09-24 12:57:11 UTCに集計コード`9368c7ad1322cc027697cc772e82815f1f4f5c6f`で、[同じraw evidence](./wasm-1.3.1/wasm-published-evidence.json)（SHA-256 `8c8640f0a63f64c5f6528ec134be2f9eb64dee41bd20b0d939073ee1a469fd17`）からJSON/Markdownを再生成した。訂正前JSONのSHA-256は`da56f42e8b6f19735798e52d78e6fa96953d115c5fb99b2173bd89240503fdff`。訂正後のcommitで画像処理や時間計測を実行したという意味ではない。再生成コマンドは次のとおり。

```bash
node test/benchmarks/wasm-upload-comparison.bench.js --reaggregate docs/history/wasm-1.3.1/wasm-published-evidence.json --previous-summary docs/history/wasm-1.3.1/wasm-upload-summary-original.json
```

Node-Wasm行のbrowser配信量を`null`にし、Chrome Worker行だけraw 1,245,431 B／gzip 407,545 Bとした。通常2シナリオの入力にはEXIF/GPS/XMP/ICCが無いため、native・Node-Wasm・Chrome Workerの`metadataStripped`を`null`に訂正した。metadata除去の根拠は別入力の`metadata-budget`ケースにのみ紐づけ、集計JSONの`metadataVerification`とMarkdownからraw evidenceへ辿れる。保存済み入力をsharpで独立に再解析し、EXIF・GPS tag・XMP・ICCの存在と記録済みSHAを突合した。Node／Chromeの保存出力はEXIF・XMP・ICCが無く、EXIF内のGPSも無い。修正後の検証コードはICCが無い入力を原因付きで拒否し、ICCだけを欠く画像による負例でも確認した。

| 実行場所とケース | ケース初回 ms | 同一optimizer/Workerのwarm中央値 ms | 出力 | budget | 結果 |
|---|---:|---:|---:|---|---|
| Node-Wasm JPEG→WebP | 3041.7 | 2380.1 | WebP 1600×1600、328,690 B | 500,000 B以下 | PASS |
| Node-Wasm PNG→JPEG | 1844.9 | 1782.9 | JPEG 1600×1600、430,035 B | 450,000 B以下 | PASS |
| Node-Wasm metadata→JPEG | 32.2 | 31.6 | JPEG 180×240、5,886 B | 30,000 B以下 | PASS |
| Chrome Worker JPEG→WebP | 3339.8 | 2143.1 | WebP 1600×1600、328,690 B | 500,000 B以下 | PASS |
| Chrome Worker PNG→JPEG | 2648.6 | 1536.4 | JPEG 1600×1600、430,035 B | 450,000 B以下 | PASS |
| Chrome Worker metadata→JPEG | 88.6 | 31.4 | JPEG 180×240、5,886 B | 30,000 B以下 | PASS |

Nodeのケース初回値はoptimizer作成後の`optimizeUpload`呼び出し時間。同じoptimizerを3ケースで使い、ケースごとに作り直していない。Node用`ImageData` shimと公開codec bytesの明示注入を使う。返却metricsの`runtime: browser`をブラウザ実行の証拠とはせず、実行したNodeプロセスで分類した。Chrome値は**ページでWorkerを新規作成する直前から最初の画像結果を受信するまで**。ページの初期ロードと入力fixture取得は含まない。各ケースで新規Worker、fresh Chrome profile、HTTP `Cache-Control: no-store`。warmは同じWorkerへの2回のメッセージ往復の中央値。ブラウザ本来の`ImageData`とWebAssembly codecを使用し、ページから入力ArrayBufferをWorkerへ渡し、返却画像bytesを受け取った。Worker helperは公開packageのものをbundleした。選択したロード方式は通常解決ではなく、Workerが5つのWasm assetsをHTTP取得して公開`wasmModules`へ明示注入する方式。通常ロードの成否は測っていない。

両runtimeとも実画像4/4生成成功、byte-budget達成は**3/4**、達成不能best-effortは3,078 BのJPEG画像を返し`budgetMet:false`、達成不能strictは**E502で期待どおり拒否1/1**。拒否は画像生成成功やbudget達成に加算しない。画像は別実装のsharpでデコードし、形式・寸法・実ファイルbytes・返却metricsを突合した。metadata出力はEXIF/XMP/ICCが無く、従ってGPSを含むEXIFも無い。`metrics.metadataStripped`だけを根拠にしていない。Node/Chromeの同ケース出力SHA-256はそれぞれ一致したが、全入力で決定性を主張するものではない。

| 初回に配信したasset | raw B | gzip B |
|---|---:|---:|
| 入口JS `main.js` | 3,368 | 1,329 |
| Worker bundle `worker.js` | 327,175 | 68,788 |
| JPEG decode Wasm | 166,470 | 63,350 |
| JPEG encode Wasm | 251,524 | 59,051 |
| PNG decode Wasm | 181,088 | 84,183 |
| resize Wasm | 34,545 | 16,893 |
| WebP encode Wasm | 281,261 | 113,951 |
| **選択した構成の合計** | **1,245,431** | **407,545** |

追加chunkは0。上表はファイルのraw/gzipサイズで、全5つを明示注入する今回の構成に必要な量。サーバがgzip配信した初回ケースの**実HTTP response body**は407,545 B（HTTP header・入力画像を除く）。3ケースでWorkerを作り直した全runのbody合計は1,219,977 B。公開package本体のインストール済みdirectoryは31,381 Bで、依存・JS/Wasm配信量とは別値。HTTP要求ごとの値とasset checksumは詳細JSONにある。

初回ケースのWorker内asset取得は31.9 ms、内部`instantiateMs`は6.1 ms、`firstEncodeMs`は251.6 ms、`totalMs`は3290.2 ms。`totalMs`はWorker起動・optimizer作成を含まず、`firstEncodeMs`はbudget探索の最初のencodeだけ、`instantiateMs`はcodec初期化を囲む値で取得時間を含まない。codec起動時間をそれ以上に分離せず、ブラウザpeak memoryも未取得。性能閾値や競合優位の判定は行わない。

測定コード・文書への検証は公開npm動作結果と区別する。構文チェック、必須runtime/versionのfail-closed統合テスト、公開version不存在・Chrome不在の負例を確認。意図的失敗runはPASS証拠に混ぜず、上記は最後の成功runだけを保存した。レビュー指摘（version既定、dirty source識別、setup FAIL保存、temporary install cleanup、テスト導線）を修正済み。PRのhosted check結果はPRで確認する。

**残課題:** #678のEdge isolate。次は公開1.3.1をlocal workerd等の実isolateで実際に画像処理し、Wasm解決・入力/出力・budget/metadata・初回処理と配信量を同等の形式で記録する。local emulator値を本番cold-startと混同しない。#703もv1.x trackerとして継続する。
