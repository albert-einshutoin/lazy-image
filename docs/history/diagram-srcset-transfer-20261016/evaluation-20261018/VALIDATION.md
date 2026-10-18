# 対象検証と納品記録

測定source revision `027b0cd9cd5a004161dc83206cdb3d14895f14d4`。測定後の集計・文書は別revisionで納品する。

| 対象 | command / evidence | 結果 |
|---|---|---|
| 保存入力・34観測・全34画像network要求・旧1171file | `node docs/history/diagram-srcset-transfer-20261016/evaluation-20261018/summarize.cjs --check` | PASS |
| 欠測と0・不正viewport・warm追加要求の脱落 | 同script `--self-test` | PASS |
| 文書・差分 | `git diff --check`、PLAN/RESULTS/DECISION/guideのセルフレビュー | PASS |
| HTTP/証拠境界・JavaScript専門レビュー | Sol medium、read-only。cold URL/page interval、warm追加要求、SW/cache、0/欠測、source/copy pin、guide寸法を確認し上記2commandを実行 | blocking P1/P2なし |
| 旧setup/observer契約 | [準備stageの実コマンドreceipt](../evidence/preparation/commands.json)、今回source hash一致 | setup8 / observer5の保存PASS。変更していない契約の再実行なし |

実command/exit/出力とhelper/summary hashは[local-checks.json](local-checks.json)。専門レビューの残る限界はChrome emulation/loopback/3trial、serverのOS handoff、RTの非packet範囲、private原本pathの抽象化、初回invalid/旧#906原因unknownであり、RESULTSと同じ境界を維持する。

今回のローカル検証は保存証拠と文書に限定した。compiler/renderer/SDK、画像再生成、native build、全test、release benchmarkは実行していない。製品API・依存・workflowの変更なし。Companion Gate、PRの最終hosted確認、merge後同SHAのmain CI/Security・artifact照合結果は、確定した対象SHAとともに#908/PRへ追記する。未実施の項目をこの表のPASSに含めない。
