# #906 検証記録

今回のhelper/docs/保存記録だけの変更。旧#902 historyの661ファイル、旧qa.js/HTML/画像、製品/API/codec/依存/CIは変更しない。初期matrix終了後の説明用summary screenshotはJPEG1440×1000、取得前後の実viewport1440×1000/DPR1が同じで、decode/画質証拠には使わない。

| 対象 | 実行・結果 |
|---|---|
| 保存2画像の全pixel decode | 既存Sharp raw toBuffer、exit0。image-checkへbytes/input/decoded hashとversions保存 |
| 旧collectorへの契約テスト | `node check-observer.test.cjs <old-qa.js>`、exit1。集約だけでは個別statusを満たせない期待失敗 |
| 新collector | `node check-observer.test.cjs`、exit0、成功/複数error/pending timeout/source変更unknown/late完了の5契約 |
| prepare | `node check-prepare.test.cjs`、exit0。旧HTML byte不変、診断5ページで画像より前に登録、既存出力拒否 |
| 記録整合 | `node check-records.cjs <repository> <history>`、exit0。旧661保護、case/trial/URL/viewport/code/HTML/probe/hash/event/分類を照合 |
| 記録の拒否 | `node check-records.test.cjs`、exit0。正常1件、source/選択画像/body/error隠蔽/event欠落の5改変を拒否。改変copyのledger hashも更新して意味の検査へ到達 |
| JS構文 | `node --check`で6ファイル、各exit0 |
| 差分 | `git diff --check`、exit0 |

実command/exitは[evidence/commands.json](evidence/commands.json)、stdout/stderrは[evidence](evidence/)に保存。VMの短い時計によるtimeout確認は実browser timeoutの観測ではない。NativeBrowserでは最終27 loadが完了し、timeout/pendingは0、制御されたEncodingErrorは2件。旧2件の実再現として数えない。

セルフレビューでは個別statusとsource変更、原本/derivativeのhash、旧証拠保護、独立fetchと元imgの区別、切断PNGのaccept、当時原因unknownの表現を確認した。初期validatorの誤拒否と初期listener登録遅れも別保存し、失敗/欠落を最終結果へ上書きしない。

専門レビュー、Companion Stop Review Gate、最終HEADの必須CI、merge後の同SHA main CI/Securityと成果物確認はIssue/PRへ記録する。過去のALLOWや旧HEADのCIを流用せず、最終CI後の新しい探索レビューを追加しない。
