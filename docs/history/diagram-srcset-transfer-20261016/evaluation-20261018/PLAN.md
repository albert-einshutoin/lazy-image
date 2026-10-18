# #908 実測stage

2026-10-18。固定契約は[元のPLAN](../PLAN.md)。実行sourceは `027b0cd9cd5a004161dc83206cdb3d14895f14d4`、baseは `795de3966ce120830a23309fa37a15910aa9ebc9`。このstageの集計・文書追加を測定時sourceへ遡及させない。

通常Chromeの所有tabでloopbackページへ移動し可視DOMを読めることを確認した。最初のMobile pilotはviewport不正のため棄却した。Chrome UI競合中は正式測定を止め、ユーザーの「専有可能（ほかのChrome UI操作を停止済み）」後に4個のcustom Device Toolbar profileを設定した。測定code/HTMLの修正はなく、新namespaceで4pilotを全てやり直してから正式24 cold＋3 prime＋3 warmを実施した。

失敗・unknownを[failures.json](failures.json)と[initial](initial/)へ保持する。原本対応は[provenance.json](provenance.json)、固定入力は[inputs.json](inputs.json)、環境・操作順・cleanupは[environment.json](environment.json)。旧1171ファイル、52,992,821 Bはsource-manifestの列挙範囲で検証する。旧history・collector・SVG・画像・失敗記録は不変。

集計の期待契約は、全34 caseとcatalog/HTML/source/server/Resource Timingの対応、正しいviewport/DPR/CSS幅、早期loadとdecode、SW非介入、cold URL未使用、prime後のcache再利用と全追加要求の分離である。失敗条件は入力hash不一致、case欠落・重複、viewport不正、src変更、decode失敗、SW不明、先行cold要求、server/Timing対応漏れ。集計値の欠測はnull/unknownとし0へ変換しない。candidate選択幅を期待値へ強制しない。

検証は保存証拠だけを読む `summarize.cjs --self-test` / `--check`、文書セルフレビュー、HTTP/証拠境界の専門レビュー1件、今回差分のCompanion Stop Gate。公開API・compiler・renderer・SDK・依存・CIの変更はしない。PRの最終hostedチェックと取り込み後の同merge SHAのmain CI/Security・artifact確認は、実測結果の根拠と分けてIssueへ記録する。
