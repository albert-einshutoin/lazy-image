# Wasm asset欠落時の診断: 修正候補のChrome Worker検証

公開npm v1.3.1の[通常ロード検証](./WASM_1.3.1_BROWSER_DEFAULT_VERIFICATION.md)と欠落負例は履歴として維持する。この文書は**未公開の修正候補**を別に検証した記録であり、公開npmへ修正が反映された証拠ではない。着手時mainは`565672d2fc57c7efa8bc7cd6b40e79ad6dfa3e98`（#855 merge）。候補packageと測定runnerのcommitは`7d0f964e26d3d0bdf6fc2dd854d6da034d4ee252`、両方とも実測時dirty=false。測定runnerの対象ファイル内容SHA-256は`79482f0591b760aa23d02838dafbbfc24acf8e71473c8017b0ced2808922efda`。候補tarballのSHA-256は`c3c72070e1c8f9fffaa89cf40f31c6e57ac2f96a838ebb7f048e5cd8f8432994`、npm lockfileのintegrityは`sha512-Y+kLxpPPFdGC5IYROXIvsy0VXr/eVedvZG+DFqqzx4196cvmVuE83qLv0VX98YMb5rlpFwVDEXyCjfeIsqjbjw==`。manifest versionは`1.3.1`のままなので、公開同versionと混同しないこと。

macOS 27.0 arm64、Node v24.2.0、npm 11.3.0、Google Chrome 153.0.8010.53、esbuild 0.25.10。候補tarballをcheckout外へpackし、別の空ディレクトリへ`file:` installした。codecは公開registry `https://registry.npmjs.org/`の`@jsquash/jpeg@1.6.0`、`png@3.1.1`、`resize@2.1.1`、`webp@1.5.0`。各取得元・integrity・license hash・実bundle入力は[正例raw evidence](./wasm-1.3.1/asset-diagnostics-candidate/positive-evidence.json)にある。Worker bundleは一時install内の候補`/worker` helperから作り、`wasmModules`を渡さず、codec自身の通常ロードを使った。`dist/`をHTTP rootとする構成で、JSは`text/javascript`、Wasmは`application/wasm`、`Cache-Control: no-store`。Chrome profileは各runで新規、各ケースの初回は新規Worker。

再実行コマンドはリポジトリrootから次のとおり。`candidate_dir`はcheckout外に置き、各run直後に`artifacts/benchmark/wasm-published-evidence.json`とconsole logを別名で保存する。`--corrupt-jpeg`は固定した6 byte (`ff d8 ff 00 00 00`)を入力にする。負例コマンドは画像処理が失敗するためexit 1を返し、rawの`verdict: FAIL`と`diagnosticValidation.status: PASS`を別々に残す。期待外の失敗は`diagnosticValidation.status: FAIL`になる。

```bash
candidate_dir=$(mktemp -d)
candidate_file=$(npm pack --workspace @alberteinshutoin/lazy-image-wasm --pack-destination "$candidate_dir" --silent)
candidate_tarball="$candidate_dir/$candidate_file"
node test/benchmarks/wasm-upload-comparison.bench.js --runtime browser --browser-load default --version 1.3.1 --candidate-tarball "$candidate_tarball"
node test/benchmarks/wasm-upload-comparison.bench.js --runtime browser --browser-load default --version 1.3.1 --candidate-tarball "$candidate_tarball" --withhold-wasm mozjpeg_dec.wasm
node test/benchmarks/wasm-upload-comparison.bench.js --runtime browser --browser-load default --version 1.3.1 --candidate-tarball "$candidate_tarball" --withhold-wasm squoosh_resize_bg.wasm
node test/benchmarks/wasm-upload-comparison.bench.js --runtime browser --browser-load default --version 1.3.1 --candidate-tarball "$candidate_tarball" --withhold-wasm webp_enc_simd.wasm
node test/benchmarks/wasm-upload-comparison.bench.js --runtime browser --browser-load default --version 1.3.1 --candidate-tarball "$candidate_tarball" --corrupt-jpeg
```

正例は2026-09-25 05:37:40 UTCに[run log](./wasm-1.3.1/asset-diagnostics-candidate/positive-run.log)と[raw evidence](./wasm-1.3.1/asset-diagnostics-candidate/positive-evidence.json)を保存した。小JPEG probe、JPEG→WebP、PNG→JPEG、metadata専用入力→JPEGの**画像生成4/4**、達成可能なbudget **3/3**、不可能な10 B best-effortは有効なJPEG 3,078 Bと`budgetMet:false`、strictは`E502`で**期待拒否1/1**。出力bytesをWorkerから受け、外側のsharpで形式・寸法・実bytes・hashを独立検査し、API返却値と突合した。入力metadataはEXIF/GPS/XMP/ICCを構造から事前確認し、専用出力のEXIF/XMP/ICC欠落（GPSはEXIF内）を確認した。通常入力にmetadata除去の実証を転記していない。

| 正例 | 出力 | 初回（Worker作成前→結果）/同一Worker warm中央値 |
|---|---|---:|
| 小JPEG probe | [WebP 320×320、17,484 B](./wasm-1.3.1/asset-diagnostics-candidate/wasm-browser-default-probe-jpeg-webp.webp) | 288.0 / 78.6 ms |
| JPEG→WebP | [WebP 1600×1600、328,690 B](./wasm-1.3.1/asset-diagnostics-candidate/wasm-browser-default-jpeg-webp.webp) | 3223.3 / 1791.2 ms |
| PNG→JPEG | [JPEG 1600×1600、430,035 B](./wasm-1.3.1/asset-diagnostics-candidate/wasm-browser-default-png-jpeg.jpg) | 2828.8 / 1598.9 ms |
| metadata→JPEG | [JPEG 180×240、5,886 B](./wasm-1.3.1/asset-diagnostics-candidate/wasm-browser-default-metadata-budget.jpg) | 77.5 / 32.9 ms |

metadataの[実入力](./wasm-1.3.1/asset-diagnostics-candidate/wasm-metadata-input.jpg)と[best-effort出力](./wasm-1.3.1/asset-diagnostics-candidate/wasm-browser-default-metadata-budget-best-effort.jpg)も保存した。asset配信量は実要求の`main.js`、`worker.js`、5 Wasmの合計raw 1,311,018 B / gzipファイルサイズ419,887 B。実HTTP転送bodyは全run合計989,648 B（繰返し取得を含む）。詳細とhashはraw evidenceにある。時間は各ケースの初回1回とwarm 2回の観測で、性能の合否判定には使わない。

| 負例（各runはfresh Chrome profile/Worker） | HTTPサーバの観測 | Workerから呼出側へ返った診断 | 画像処理 / 診断検証 |
|---|---|---|---|
| [正常JPEG、JPEG decoder Wasm欠落](./wasm-1.3.1/asset-diagnostics-candidate/mozjpeg_dec-evidence.json) | `/mozjpeg_dec.wasm` **404** | `E131`、`CodecError`、`recoverable:false`。`message`はJPEG decoder初期化またはdecode失敗、`recoveryHint`は配置・Network・入力の順に案内 | FAIL / PASS |
| [正常JPEG、resize Wasm欠落](./wasm-1.3.1/asset-diagnostics-candidate/squoosh_resize_bg-evidence.json) | `/squoosh_resize_bg.wasm` **404** | `E503`、`CodecError`、`recoverable:false`。resize段階と該当Wasmを案内 | FAIL / PASS |
| [正常JPEG、SIMD WebP encoder Wasm欠落](./wasm-1.3.1/asset-diagnostics-candidate/webp_enc_simd-evidence.json) | `/webp_enc_simd.wasm` **404** | `E300`、`CodecError`、`recoverable:false`。WebP encoder段階と選択variantを案内 | FAIL / PASS |
| [破損JPEG、Wasm配信正常](./wasm-1.3.1/asset-diagnostics-candidate/corrupt-jpeg-evidence.json) | `/mozjpeg_dec.wasm` **200**、`application/wasm` | `E131`、`CodecError`、`recoverable:false`。`message`はdecode失敗で、404/asset欠落を断定しない | FAIL / PASS |

各負例の[CLI log](./wasm-1.3.1/asset-diagnostics-candidate/)はexit 1を記録し、raw evidenceの`browserResults.expectedFailure.workerError`に**返却された全文**、`wasmRequests`と`requests`に要求URL・statusを保存した。URL/statusは**検証用HTTPサーバ**から得た情報であり、packageが観測・返却した情報ではない。packageは失敗したcodec段階と元例外textを把握できるが、codec内部fetchのHTTP statusを直接読めないため、`message`は取得失敗と画像処理失敗を断定しない。利用者は`recoveryHint`に従い、DevTools NetworkでURL/status/HTMLではないWasm応答を確認する。正常配信なら入力破損やcodec処理を調べる。原因不明のthrow値がWorker転送でさらに失敗しないことと、明示module compile失敗のcodec別診断はpackageの限定テストで確認した。

公開版1.3.1のasset欠落時は、同じHTTP 404を経て`E131`ながら`recoveryHint`が入力破損確認だけだった。本候補は`code`・`category`・`recoverable`を維持して案内先を改善した。**公開npmの修正版検証は未実施**。新versionへ反映後、その公開packageを隔離installしてChrome通常ロードの正例・欠落・破損入力を再確認する必要がある。
