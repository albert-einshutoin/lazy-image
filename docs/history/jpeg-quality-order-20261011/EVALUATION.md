# JPEG quality設定順序の修正と同条件比較 (#885)

親 [#703](https://github.com/albert-einshutoin/lazy-image/issues/703) の子 [#885](https://github.com/albert-einshutoin/lazy-image/issues/885)。
JPEG normal modeで指定qualityが出力DQTへ反映されるよう、共有エンコード経路の設定順序を修正した。
画質・容量・処理時間の優位を保証する変更ではない。公開API、既定quality数値、codec/backend、依存、CIは変更していない。

## 再現と修正

修正前mainは `9086814a7295fef7649187850222c96b65862a36`。
既存の「qualityでsizeが変わる」テストは非空・JPEG識別子だけを確認し、この欠陥を検出できなかった。
これを生成JPEGのpre-SOS DQTを検査するテストへ置換した。同じsmoothing=0のq90/q100で、
normal modeの2つのDQTが同一となり、**修正前はexit 101** ([ログ](checks/red-dqt.log))。
これはmain＋未コミットの回帰テスト草稿（format前）での実行で、製品コードは未修正だった。
テストcommit `059fcb5311ddce4fa9e07b135ecd4eba555b63c7` も製品経路は旧実装のままである。

固定mozjpeg0.10.13の`set_scan_optimization_mode`は`jpeg_set_defaults()`を呼ぶ。
mozjpeg-sys2.2.3はquality75、scan、entropy、smoothing、colorspace/chroma等を初期化する。
normal modeのこの呼び出しを`Compress::new`の直後へ移し、size/color/quality/chroma/progression/
optimize/smoothingをその後に確定した。`AllComponentsTogether=0`はdefaultsのscan modeと一致する。
fast modeには新しいdefaults呼び出しを入れていない。ICCの後段埋め込み、private metadata処理、
quality validation、panic/error分類、容量探索/strict判定の処理は変更していない。
quantizationの最適化をentropy codingの効果とする誤ったコメントも削除した。

修正後はq90/q100のDQTが異なり、q100の全量子化step=1を確認する。
サイズ/SSIMの単調性を仮定せず、q100のpixel losslessとも解釈しない。
frame/decode検査はnormal progressive(C2)、fast baseline(C0)、両方4:2:0と寸法を修正前後で確認した。
normal/fast ICCテストは抽出ICC bytesの完全一致まで検査する。

## 同一マシン・同一条件の新規測定

[機械可読比較](comparison.json)、[before provenance](raw/before/provenance.json)、
[after provenance](raw/after/provenance.json) と各入力/engineの全候補・3時間sampleを保存した。

| 対象 | source SHA | native binding SHA-256 |
|---|---|---|
| 修正前 | `059fcb5311ddce4fa9e07b135ecd4eba555b63c7` | `74c5a0179ffa03609713e95cefc1ffd25e47571954b3085d27e3d1f093f1e276` |
| 修正後 | `d66c451021a96454ffedbecdfdc112b19331d4a4` | `10497403b2fd60c1f01e929fbbe8a3c6dabb5d448d9927a15d99e771d2385174` |

両方clean checkout、ローカルnative **1.4.3** build。公開npmの測定ではない。
Apple M4/10 CPU/32 GiB、darwin arm64、Darwin27.0.0、Node24.2.0、Rust1.96.0、NAPI CLI3.9.0。
sharp0.35.4/libvips8.18.6/mozjpeg0826579。Cargo/npm locks、runner/helpers、fixture/license hashesが一致する。
最初のbuildはローカルCLI3.7.2で生成差分が出たため測定に使用せず、生成差分を戻して`npm ci`でlock版を復元した。
旧buildのexit0と復元後buildのexit0を各ログに分離して保持し、npm ciの既存dev advisory表示も保存した。
依存定義・lockは変更していない。保存した各sourceのencoder製品部分hashをcomparisonに記録した。
後続のICCテスト強化と記録追加は製品部分を変更せず、この修正後測定の適用範囲を変えない。

既存quality workerをそのまま使用し、coffee、ui-1、metadata-1のJPEGのみを順次測定した。
共通referenceは最大320px、auto-orient、白背景合成PNG。各quality 12候補×3 encode。
同じマシンでもbefore/afterは別の時点であり、CPU負荷や熱・周波数を制御した統計実験ではない。
両engineのreference digestが一致し、Sharp候補のbytes/hash/SSIM/PSNRも前後で一致した。

| 入力 / q90 | SSIM 修正前→後 | bytes 修正前→後 | encode p50 ms 修正前→後 |
|---|---:|---:|---:|
| coffee | 0.928983→0.969907 | 11,722→20,721 | 6.249→9.051 |
| ui-1 | 0.987631→0.994299 | 4,731→6,931 | 3.692→4.451 |
| metadata-1 | 0.987017→0.993932 | 5,215→7,452 | 3.292→4.458 |

同じ要求qの比較で、修正前はその要求が上書きされていた。等しい知覚品質でのコスト比較ではない。
修正前は3入力ともq90/q100の出力hash・bytes・SSIMが同一だったが、修正後は異なった。
q100ではcoffeeの59,476 B/33.154ms、ui-1の13,455 B/7.455ms、metadata-1の12,307 B/6.237msとなり、
高quality指定の容量・時間増を隠していない。全画像での容量削減や速度改善は主張しない。

共通Sharp q80のSSIM下限達成は **0/3→3/3**、byte cap達成は **3/3→3/3**。
修正後の下限達成候補は3入力ともq85で、容量は16,078 / 5,823 / 6,328 B。
それぞれSharp anchorの13,698 / 5,303 / 5,893 Bより大きい。
同cap内の最良SSIMは修正後も3入力ともSharp未満。下限とcapを同時に満たす候補は前後とも0/3。
二つの独立選択が達成されても同時達成やcodec優位へ読み替えない。処理失敗・測定不能は保存候補で0。

sampled grid RSSはcoffee126.63→131.72 MiB、ui-1 143.59→135.02 MiB、metadata-1 136.31→137.36 MiB。
reference作成後のgrid全体を10ms samplingし、両workerが両libraryをloadする。
encoder単体allocation、exact peak、OOM耐性ではない。3sampleのnearest-rank p90は最大sampleで、母集団p90ではない。
白合成したmetadata起点のcodec比較は、ICC/private metadata保持の検証へ転用しない。
[#883の歴史評価](../native-compiler-cost-20261011/EVALUATION.md) はLinux x64/native1.4.2の別証拠として保持する。

## 再実行コマンドと対象検証

各測定sourceの別のclean checkoutで`npm ci`→`npm run build`後、同じrunnerを使用する。
出力は既存証拠へ上書きせず、新しい空directoryへ保存する。各provenanceに実コマンドとexit0を記録した。

```bash
node test/benchmarks/quality-matched.bench.js --worker 3 jpeg lazy
node test/benchmarks/quality-matched.bench.js --worker 3 jpeg sharp
node test/benchmarks/quality-matched.bench.js --worker 5 jpeg lazy
node test/benchmarks/quality-matched.bench.js --worker 5 jpeg sharp
node test/benchmarks/quality-matched.bench.js --worker 8 jpeg lazy
node test/benchmarks/quality-matched.bench.js --worker 8 jpeg sharp
```

- `cargo test --no-default-features test_encode_jpeg_quality_changes_quantization_tables -- --nocapture`: 修正前FAIL/exit101。
- `cargo test --no-default-features test_encode_jpeg_mode_frame_settings -- --nocapture`: 修正前1成功/exit0。
- `cargo test --no-default-features jpeg`: 修正後43成功/exit0 ([ログ](checks/green-rust-jpeg.log))。
- `cargo test --no-default-features with_icc`: ICC equality強化後7成功/exit0 ([ログ](checks/green-rust-icc.log))。
- native対象6ファイルはすべてexit0 ([実結果](checks/targeted-js-results.json)):
  `node test/integration/{basic,error-codes,target-bytes-file-native,artifact-compiler,metadata-options,exif-roundtrip}.test.js`
  （各ファイルを別プロセスで実行。brace展開を単一node呼び出しへ渡さない）。
  Buffer/file・quality不正入力、targetBytes、strict失敗時の既存file保護、JPEG compiler E300時のdirectory非公開、
  EXIF/GPS opt-in/strip契約とJPEG出力のmetadata経路を確認した。

専門レビュー、最終Stop Review Gate、完全検証と納品結果は[検証記録](VALIDATION.md)を参照する。

## Golden snapshotの意図した出力変更と旧容量期待の未達

最初の完全`npm test`はexit1で停止し、`test:bench:release`はその時点で未実行だった
([初回実結果](checks/full-local-initial-results.json)、[失敗ログ](checks/full-npm-test-initial.log))。
失敗はJPEG q82 golden caseのpixel hash変更。他5ケースのhashは不変だった。
保存した修正前bindingをpublic indexへその検証プロセス内だけで接続し、同一fixture・既存golden helperで比較した。
[修正前](checks/golden-before-probe.json) は旧hashを直接再現し、[修正後](checks/golden-after-probe.json) は
新hash `dad8e345f4403a2e5c5ae2d139a9e77bf06b5ada853f6b770529b5d801ea583e` と1200×1200を確認した。
fixture hash/reference hashは一致し、binding hashを各記録に保持した。

SSIM 0.994862→0.997755、PSNR41.231→47.575 dBで、既存のSSIM>=0.99/PSNR>=38条件を満たす。
bytesは95,964→133,070 B、Sharp reference135,970 Bに対する容量比は0.705773→0.978672。
**旧sizeRatioMax=0.95（5%優位）は未達**であり、probeの`checks.sizeRatio:false`を保持している。
最初の修正後probeはこの条件のassertでexit1となり、その後の値抽出は診断の保存で、旧条件のPASSではない。

q82が上書きされていた出力のpixel snapshotを修正後へ更新し、このJPEG caseの相対容量優位assertだけを削除した。
新しい都合のよい閾値を設定せず、同qの全入力優位をquality修正の契約にしない。
既存の画質閾値・寸法・pixel hash検査、他5ケースのhash/guard、actual targetBytes/strict非公開・非保存条件は保持した。
容量比はgolden実行の表示と本評価へ残し、旧優位未達を品質修正の成功へ加算していない。
対象goldenを再実行し6ケース成功/exit0 ([ログ](checks/green-golden.log))。
