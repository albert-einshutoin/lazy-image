# #908 準備状況

**評価は未完了。初回転送量の削減・画像選択・decodeの新しい実測結果はまだない。**

2026-10-16。開始時main/local/remoteは `795de3966ce120830a23309fa37a15910aa9ebc9`、tracked worktree clean。重複する未完了Issue/PRがなく#908を作成した。#902/#904/#906はCLOSED、#703はOPENを維持する。

旧#902/#904/#906 historyと原本SVGの1,171ファイル、52,992,821 Bを `source-manifest.json` で固定。WebP3件は保存済みの公開npm1.5.0成果物で、今回はcompiler/renderer/SDKを実行していない。公開生成のsource・manifest・consumer identityは旧#904の記録を参照する。現在のローカルbuildや別versionの生成物へ置換しない。

新helperはmanifestからURL/width descriptorを生成し、初回の24caseを互いに未使用のasset URLへ分離する。取得済み対照のprime/responsiveだけは同じ候補URLを共有する。#906の個別decodeと早期イベント登録scriptをbyte不変で再利用し、probe属性を付けない。補助scriptは可視DOMへの記録とService Worker/Resource Timingの観測だけを行う。loopbackはcatalog許可fileだけを配信し、画像はcacheable、HTML/scriptはno-store。配信する3画像は合計345,298 Bで、body bytesや転送量の実測ではない。

## 実施済みの対象チェック

- `node check-setup.cjs`：helper実装前はexit1。その結果をGit外に保持。実装後はexit0、8契約。24初回のURL分離、prime/warm3組、pilot4ページ、manifest/CSS/早期observer、画像hash、既存出力保護、source不一致拒否、使い捨てloopback HTTP/記録を確認した。HTTP304のbody0とContent-Lengthなし、入力変化時の500も対象に含む。
- `node check-observe.cjs`：初期exit0は4契約。専門レビューでscriptを含む全resourceと画像転送量のscope表現の不一致が1件あり、画像候補だけの配列を要求するcheckを追加して修正前exit1を確認した。修正後exit0、5契約。個別status保持、観測0と未対応null、script除外と画像request重複保持、未完了pending、Service Worker観測拒否unknownをVM fixtureで確認した。実Chromeの結果ではない。
- `git diff --cached --check`：exit0。

原本test stdout/stderrは今回のtask-local directoryに保存。browser測定codeのfreeze revision、正式HTML/hash、実行コマンドと最終対象チェックの公開用対応は、pilot前に確定する。未作成・未確認の証拠を過去のreceiptとして作らない。

最終準備チェックは `evidence/preparation/commands.json` に実command、開始/終了、exit、実行前後のsource hashを保存した。実行はbase HEAD上の未コミットsourceで、commit実行と偽らず、hash snapshotへ結び付けている。初期チェックの記録は未freezeの準備履歴で、最終sourceの成功証拠とは分ける。

準備コードの専門レビュー（Sol medium、READ_ONLY / STATIC_ONLY）は1件のBLOCK後、該当3ファイルだけの修正・再確認でALLOW。全resourcesと候補imageRequestsの区別、script除外、重複保持を確認した。これは実browser結果・最終delivery Gate・CI・merge・Goal完了の判定ではない。

## 現在の障害と残作業

CUAの通常Chrome session setupが `Unable to load browser request-header policy. Retry the browser command.` で失敗し、通常retryと準備後の再確認でも同じ失敗になった（3試行）。APIの説明読取りは可能だが、browser操作は未開始。ブラウザ全体のcache消去、security policy回避、別の制御経路への置換は行っていない。この失敗は画像decodeの失敗・転送削減なしの観測ではない。

残りは実browserの4 pilot、正式24初回表示、Mobile DPR2のprime3＋対象3、actual UA/viewport/DPR/zoom、元request/応答・追加variant・cache根拠の取得。その後に条件別RESULTS/DECISION、導入ガイド、レビュー/Gate、最終HEAD CI、1PRのmerge、同merge SHA main CI/SecurityとIssue完了を行う。必須観測ができるまでGoalと#908を完了しない。旧#904のwarm結果を今回の初回結果へ読み替えない。
