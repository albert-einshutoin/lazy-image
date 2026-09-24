# 公開Wasm 1.3.1: local workerdでのEdge isolate実測

**PASS。** 2026-09-24 15:38:50 UTCに公開npmの`@alberteinshutoin/lazy-image-wasm@1.3.1`を実local workerd isolateで動かした。着手時mainは`0ab1b89035c5dca10b603bb4beb7ac6c8c03fd62`（#853 merge）、測定コードは`aa99d7ebb46ff8e16d613f3bb0e3c498c1fa508f`、dirty=false、測定に関わる6ファイルの内容SHA-256は`cb86fd2d5f9bb9bc8ceb87c80d1a724b64a34491e4ed96bace5921fd267dbfee`。#853の[Node/Chrome実測](./WASM_1.3.1_VERIFICATION.md)は別日時・別SHAの既存証拠として保持する。

macOS 27.0 arm64、Node v24.2.0、npm 11.3.0、`workerd@1.20260924.1`（実行ファイル出力`workerd 2026-09-24`）、esbuild 0.25.10。compatibility dateは`2026-09-24`、追加flagsなし。`https://registry.npmjs.org/`からcheckout外の空の一時ディレクトリへ通常installした。本体tarballのintegrityは`sha512-80yry+i323threLO1+cl4BkxBIknDBWPgRiQ08Jt3Z98cUfJNZV5WKKCrsfwpcd8fCKC7EhuaDkXrLAxXsxcOw==`。公開本体・JPEG/PNG/resize/WebP codec依存のversion、resolved URL、integrity、実bundle入力は[raw evidence](./wasm-1.3.1/edge-workerd/wasm-published-evidence.json)に保存した。

同じraw evidenceに各packageのmanifest SHA-256・宣言license・同梱LICENSEファイルのSHA-256、同梱されない場合はcanonical licenseの出典とSHA-256、選ばれたplatform binary packageのversion/URL/integrityと実行ファイルSHA-256を保存した。公開Wasm本体は[v1.3.1のLICENSE](https://github.com/albert-einshutoin/lazy-image/blob/v1.3.1/LICENSE)のSHA-256 `ff1b6da07c1a09446754bf5e0fe61a788fc6815c0ea0517d385df0b725b2b539`を照合した。esbuildの同梱`LICENSE.md`とplatform binary packageが参照する同ファイルは`b40ec5baec7bb34fa5b1c09521fa3cd52d5fad7adafed74932a2010d3612a681`、実バイナリは`921b19d2a6e983de6aa861582476e4f983c8509cb40c462e572ce64ca1bcb5be`。npmのworkerd本体・platform packageにはLICENSEファイルが無いため、宣言`Apache-2.0`と[version固定の上流LICENSE](https://github.com/cloudflare/workerd/blob/v1.20260924.1/LICENSE)のSHA-256 `0d542e0c8804e39aa7f37eb00da5a762149dc682d7829451287e11b938e94594`を取得・照合した。実行した`@cloudflare/workerd-darwin-arm64`バイナリを公開npmからのインストール済みバイナリと照合し、両方のSHA-256は`354615e8d5ccbc2ab9afff5ec7f9a3a6e21f83bb27c314c4c65685d1fbaf984c`。

再実行コマンドはリポジトリrootで次のとおり。`workerd`は測定用の一時installへ追加され、終了時に一時ディレクトリを消す。実行ログと画像は`artifacts/benchmark/`へ出力される。今回保存した[実行ログ](./wasm-1.3.1/edge-workerd/wasm-published-run.log)、[summary JSON](./wasm-1.3.1/edge-workerd/wasm-upload-summary.json)、[Markdown](./wasm-1.3.1/edge-workerd/wasm-upload-summary.md)、[出力画像](./wasm-1.3.1/edge-workerd/)は同じ成功runのコピーである。JSON内の`artifacts/benchmark/`は実行時の出力先を指し、本ディレクトリには同名の保存コピーがある。

```bash
node test/benchmarks/wasm-upload-comparison.bench.js --runtime edge --version 1.3.1
node test/benchmarks/wasm-edge-failure-check.mjs 1.3.1
```

保存済みsummaryの対象runtime表示は、2026-09-24 15:51:40 UTCに集計コード`a468a517cdcb0e956b30383a07c2d06f6106b29e`でEdge単独へ訂正した。JSONの`aggregation`は元のraw evidence SHA-256 `898d47d1d29cf1b49433a36f040c9a73b091f1e0489659688632c26db287ad13`と訂正前summaryのhashを記録する。元の画像処理は15:38:50 UTC・測定コード`aa99d7e`のままで、画像や時間値を再測定していない。再実行コマンドは上記の公開npm Edge runである。

測定コードは公開packageの`/edge`入口を一時install内からesbuildでbundleし、workerd設定のstatic Wasm module importで得た**実`WebAssembly.Module`** 5件を公開`wasmModules`へ渡した。NodeのImageData/DOM shimもcodec差し替えも追加していない。`/health`は5件の型、isolate ID、optimizer作成回数0を確認し、画像処理を先行させない。各ケースは新規workerdプロセスとisolateで最初の画像を処理し、その後同じisolate ID・optimizer作成回数1のまま2回warm実行した。公開版の`metrics.runtime`だけではなく、実workerd起動、HTTP応答、bundleの公開`edge.js`入力をEdge実行の証拠とした。

入力は#853と同じ5000×5000 JPEG・PNGおよびmetadata専用JPEG。fixtureのSHA-256、変換policy、品質範囲、byte-budgetは[raw evidenceのfixtures](./wasm-1.3.1/edge-workerd/wasm-published-evidence.json)にある。metadata元画像`metadata-1.jpg`は[release corpus manifest](../../test/benchmarks/corpus/release-manifest.json)記載のプロジェクト生成MIT fixture（SHA-256 `ff7c61bc5ba08c18ce0c57fa49d61dd6af0d5d1264f9dc398715af2d6ab01b6d`）。EXIF/GPS/ICCを保持しXMPを追加した[実入力](./wasm-1.3.1/edge-workerd/wasm-metadata-input.jpg)はSHA-256 `29a8b5ce307f40c8fa546da2e3773341451bcfc60106eb62fb71d8a101dfce70`で、4項目の存在を画像構造から先に確認した。

| 実行ケース | 呼び出し側の起動込み初回 / 最初の画像request / warm中央値 ms | 独立にdecodeした成果物 | 判定 |
|---|---:|---|---|
| 小JPEG→WebP probe | 227.0 / 195.5 / 120.5 | [WebP 320×320、17,484 B](./wasm-1.3.1/edge-workerd/wasm-edge-probe-jpeg-webp.webp) | PASS |
| JPEG→WebP、resizeあり | 3882.7 / 3853.1 / 2594.7 | [WebP 1600×1600、328,690 B](./wasm-1.3.1/edge-workerd/wasm-edge-jpeg-webp.webp)、500,000 B以下 | PASS |
| PNG→JPEG、resizeあり | 3040.0 / 3009.2 / 1944.1 | [JPEG 1600×1600、430,035 B](./wasm-1.3.1/edge-workerd/wasm-edge-png-jpeg.jpg)、450,000 B以下 | PASS |
| metadata→JPEG | 90.2 / 60.5 / 38.4 | [JPEG 180×240、5,886 B](./wasm-1.3.1/edge-workerd/wasm-edge-metadata-budget.jpg)、30,000 B以下 | PASS |

全生成画像をisolateから実bytesで返し、外側のsharpで独立decodeした。形式・寸法・実ファイルサイズ・SHA-256を返却値と突合した。通常のJPEG/PNG入力にはEXIF/GPS/XMP/ICCが無いため、その2行の`metadataStripped`は`null`で、除去実証とは数えない。metadata専用ケースは入力にあったEXIF/GPS/XMP/ICCに対し、出力のEXIF/XMP/ICCが無いことを確認した。GPSはEXIF内のため、EXIF自体の消失で出力にも残らない。

画像生成は**4/4**、達成可能なbudgetは**3/3**。達成不能な10 Bの`best-effort`は[有効なJPEG 3,078 B](./wasm-1.3.1/edge-workerd/wasm-edge-metadata-budget-best-effort.jpg)を返し、`budgetMet:false`でbudget未達として集計した。`strict`は`E502`（`ResourceLimit`）で期待どおり**1/1拒否**。拒否を画像生成やbudget達成へ加算していない。各出力のhashと返却metricsはraw evidenceにある。

呼び出し側のNode `performance.now()`を主な時計とし、各ケースの**新規workerd起動前→初回画像bytes受信**、起動後の**最初の画像request→bytes受信**、同一isolate・optimizerのwarm 2回を記録した。表のwarmは2回の中央値、coldは1回の観測値で、ばらつきの統計的推定ではない。起動済み条件は画像を実行しない`/health`の成功。内部`totalMs`はoptimizer作成やworkerd起動を含まず、`firstEncodeMs`はbudget探索の最初のencodeだけである。probeの内部`totalMs`は[raw evidence](./wasm-1.3.1/edge-workerd/wasm-published-evidence.json)に保存した。

ローカルworkerdの`performance.now()`はprobe・PNG・metadataケースの200万回CPUループ中に2〜3 ms進んだ。ただし大JPEGケースは画像3回に成功した後、追加の`/clock`診断requestが接続終了となり、その時計診断は理由付き`unavailable`と記録した。呼び出し側の初回・warm時間と画像検査には影響しない。原因は未特定で、内部msを本番Cloudflareの壁時計やCPU課金時間へ換算しない。peak memoryは取得していない。

| static Edge配布asset | raw B | gzip B | SHA-256 |
|---|---:|---:|---|
| `edge-worker.mjs` | 324,775 | 68,391 | `6911fd7f064181d785b6945be376088f1c1d7af216ad3b58cba6b1cae51c7209` |
| JPEG decode Wasm | 166,470 | 63,350 | `a7c4b12169817e779ff4af137981393ae924944e167ad1bd95747c9199162d3e` |
| JPEG encode Wasm | 251,524 | 59,051 | `24d4177f1c4963e2058b107189249651c61fdef125570e79b1dfb63c8bb49326` |
| PNG decode Wasm | 181,088 | 84,183 | `263d6e658808a74b72a1a99c5cc1d619237e70c150db6e41d5d84d3d117ab9be` |
| resize Wasm | 34,545 | 16,893 | `5b1f702d502c4d0a70b99f78691bd554d566ba95859e4c57af5435955a1d74a5` |
| WebP encode Wasm | 281,261 | 113,951 | `b6085bb6702f144e9dc6016d58d230b34a84976bf0d080b7390b4b4b137d6ab7` |
| **今回の構成の合計** | **1,239,663** | **405,819** | — |

追加chunkはない。表は全5 moduleを静的に注入する**この構成の配布ファイルサイズ**であり、ブラウザHTTP転送bodyやpackage directory全体のサイズではない。選んだ処理に必要な配布量を示す。通常の暗黙Wasmロード、Cloudflare本番cold-start、課金CPU、他のEdge runtime、性能の合否閾値や競合優位は検証していない。

[負例ログ](./wasm-1.3.1/edge-workerd/fail-closed.log)ではworkerd実行ファイル欠落をBLOCKED、公開npmとhashが異なる実行ファイルの指定・Wasm module欠落・不正画像による処理失敗をFAILとして記録した。[公開runnerの非zero例](./wasm-1.3.1/edge-workerd/missing-workerd-cli.log)も保存した。必須ケースを実行できない状態や失敗をPASSにはしない。今回の公開v1.3.1は選定したlocal workerd構成の受入条件を満たすため、PRのレビュー・必須チェック・merge後に#678をcloseできる。#703はv1.x trackerとして継続する。
