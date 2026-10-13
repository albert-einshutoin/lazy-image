# #904 対象検証とレビュー

2026-10-14。今回のSVG・guide・評価helper/保存記録のみ。製品コード、公開API、codec、依存、CIは変更していない。画像生成の測定helperは`a8aa32716c8ce9fa4078808a9424e12969171a7f`の4ファイルで、現ファイルとのbyte一致をGit保存内容へ確認した。後の記録チェック追加を測定し直した結果として扱わない。

## 実行結果

`REPOSITORY`/`HISTORY`/`ORIGINAL_PHASE`の意味は[REPRODUCE](REPRODUCE.md)。下記は各終了状態を確認した。

| 対象 | 実command / 確認 | 終了・結果 |
|---|---|---|
| 出力/provenance | `node "$HISTORY/check-outputs.cjs" "$REPOSITORY" "$HISTORY/evidence" --check` | exit0、4 calls/24 artifacts、B18/18適合、A/B画像hash一致 |
| browser/原本対応 | `node "$HISTORY/check-browser.cjs" "$REPOSITORY" "$HISTORY" "$ORIGINAL_PHASE" --check` | exit0、完全な90 pose、DPR/幅/pan、crop/原本JPEG/capture JSON hash一致 |
| 拒否の回帰確認 | `node "$HISTORY/check-bindings.test.cjs" "$REPOSITORY" "$HISTORY" "$ORIGINAL_PHASE"` | exit0。正常2件exit0、取り違え15件それぞれexit1 |
| JS構文 | `node --check <each history .cjs / consumer .mjs>` | 11ファイル各exit0 |
| 公開copy照合 | `LOG_REDACTIONS.json`の396 fileのbytes/SHA-256を実copyへ照合 | exit0 |
| private path | 公開text266 fileの既知利用者home/volume pathを検索 | exit0、該当0 |
| HTTP配信 | loopbackのSVG/PNG/WebP/JPEG10 assetをfetch、保存previewのbody bytes/hashへ照合 | exit0、10/10 status200。browser転送量benchmarkとは別 |
| whitespace | `git diff --check` | exit0 |

[取り違えチェック結果](evidence/checks/binding-tamper-test.json)、[HTTP body/hash結果](evidence/http-check.json)を保持。取り違えは使い捨てコピーだけを変更し、実測原本・画像・旧#902・準備記録は変更していない。

## セルフレビュー・専門レビュー

SVGの6工程と赤い失敗分岐、guideのlocal artifact commitとアプリのHTML/外部配信責任をAPI/compiler/例へ照合済み。Desktop通常とMobile拡大の文字/接続、PNG対照と最終WebP/JPEG、DPR/75%撮影/decoded-file pixelの境界を確認した。全候補成功は今回の1点・環境だけに限定し、旧#902のdecode2件は未解明を保持している。

公開cropとsource correspondenceの専門レビューは`gpt-5.6-sol` mediumを1件使用（native windowの私有UIを公開資料へ混入させない境界のため）。base `ef5e1bf3addddc41de4ce5c404d64a9fe4c54b09` / 初回head `5d95f38d7a27084e434ef1fbefa797d2713e16c5`でBLOCK。指摘は2件：browserのpose/pan/原本対応、測定のhelper/input/policy/package provenanceチェックが不足していた。

両指摘を同時に修正し、完全pose集合と位置、原本JPEG＋capture JSON＋cropのhash、測定code/command/source/native/lock/JS/input/policy/exit/file inventoryと実manifestを照合した。90 descriptorへ追加したcapture JSON hashは既存原本と画像の実hashを確認してから保存し、以前のdescriptorはローカルに保持した。画像、撮影、処理時間の再生成はない。

指摘範囲の最終専門レビュー、今回のHEADへのCompanion Stop Review Gate、最終hosted CIとmerge SHAはPR/Issueの納品記録へ結び付ける。過去のALLOWや旧HEADのCIを今回の成功と扱わない。公開証拠のファイル一覧/hashは[evidence-hashes.json](evidence-hashes.json)。原本画面は私有UIを含み得るためGit外保管で、取得できない第三者の原本対応照合は未確認とする。
