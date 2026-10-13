# 公開後記録の対象検証

2026-10-13。対象は公開証拠の整合・集計・公開用derivativeと再利用helperであり、製品アルゴリズム・CI・versionは変更していない。公開runtimeの実行結果は個別raw・log・exitへ結び付け、本記録の確認を新しい画像処理実行とは呼ばない。

- `node --check`：verification-toolsのcompare-registry.mjs、summarize-smoke.cjs、module-smoke.mjs、各exit 0。
- summarize-smoke.cjsを採用12公開report・12tag候補reportの別copyへ実行し、各exit 0。正確な6 platform×Node22/24、source=release commit、実binding/API/CLI/artifacts、q30/1 MiB上限、invalid29/101/string30 E400と非公開、commandの実exit/signal/errorを検査した。保存済み原本summaryを上書きしていない。
- 実tag候補8 tarballと公開registry取得8 tarballの実bytesからSHA-256・dist.integrityを再計算。manifest/provenanceのsource・tree・tag-run、全8name集合・version/latest、Wasm展開12file一致を検査した。helper guard追加後の検証にregistry問い合わせの再実行は加えていない。
- CJS/ESMの実出力2件とtag候補binding hash、Chrome出力5件のbytes/hash、metadata入力・除去、10 B best-effortのbudgetMet=false、strict E502を検査した。CJS/ESM install/run・browser npm ci/run・registry成功attempt・公開集計の実exit 0を読み返した。初回registry/古いartifact集計のexit 1も保持した。
- 公開derivativeのSHA-256をLOG_REDACTIONS.jsonと照合し、外部原本のhashも再計算一致。public textのlocal home/workspace/temp/cache pathとtoken形式のscanは成功。初回scanはnpm cache pathの残存2ファイルで失敗し、cache rootを置換してから再検証した。公開logの行末空白も除去し、ledgerに変更数を記録した。git diff --cached --checkの初回指摘後、再確認はexit 0。結果・URL・source/hash・診断を削って成功扱いしていない。

[対象検証log](record-validation.log)・[実exit 0](record-validation.exit)を保存。小さなstdlibの照合処理を作業時に用い、新依存・汎用benchmark基盤は追加していない。主要な再確認は保存JSONを読むチェックと、既存summarize helperの実行で再現できる。公開runtimeを再測定する場合は、親文書の実行手順と固定release sourceを使い、新しい出力先へ保存する。

## helper専門レビューと指摘解消

公開記録helper3件の専門レビューはgpt-5.6-sol / medium、READ_ONLYで実施した。初回BLOCKはcompare-registryがmanifestの正確な8package集合を検査せず、欠落した入力でも「all8」と表示できる点だった。registryアクセス前へ集合検査を追加。PATHを空にした欠落7件・重複8件の負例は、どちらも期待assertでexit 1となり、registryへ到達する前に拒否した（[負例raw](manifest-negative-check.json)）。同じ指摘範囲だけの最終レビューはALLOW、touched=[]。コードの別経路・fallback・新依存は加えていない。

セルフレビューでは、release対象commitと記録commit、準備候補とtag候補、候補と公開runtime、正常受入と期待拒否、report PASSと実process終了値、初回と再試行artifactを分離して確認した。Companion Stop Review Gate・記録PRの最終HEAD・必須CI・merge対応は今回のIssue/PR記録へ追記する。
