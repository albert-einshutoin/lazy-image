# 保存証拠の再集計と新規観測

repo root、Node22以降。保存証拠の再集計はnetwork/browser/native/SDKを呼ばない。旧source-manifestの列挙1171件、測定source、公開copy、catalog/HTML/保存WebPのbytes/SHAを先に検証し、34caseの全server候補requestを対応付ける。

```sh
node docs/history/diagram-srcset-transfer-20261016/evaluation-20261018/summarize.cjs --self-test
node docs/history/diagram-srcset-transfer-20261016/evaluation-20261018/summarize.cjs --check
git diff --check
```

引数なしはsummaryをstdoutへ生成する。`--check`は保存summaryとの完全一致を要求する。`--self-test`は0と欠測の区別、実際の不正viewport pilotの拒否、warm追加684entryを落とした場合の拒否を検証する。入力不正や対応漏れはError、数値欠測はnull/unknownで全体statusもunknownとなる。期待選択幅へ書き換えたりnetworkで穴埋めしたりしない。

新規観測を行う場合は[元の手順](../REPRODUCE.md)と[PLAN](../PLAN.md)に従い、別の存在しないpreview directoryとrandom namespaceを作る。稼働通常Chromeのfull versionをemulation前に記録し、画像のないidle上でzoom100%・Disable cache unchecked・No throttlingを設定する。Device ToolbarのDesktop1440×1000/DPR1,2、Mobile390×844/DPR1,2をcustom profileで設定し、実DOMのviewport/DPRとCSS960/342をcaseごとに確認する。Fit to windowはbrowser zoomと分ける。

4pilotが成立してから24coldと3prime→warmを観測する。変更前にidleへ移動し、同じtabのviewport切替による追加取得を避ける。primeresponseの完了・cacheable headersを保存してから同URLのwarmへ移動する。global cacheを消去しない。実際のUI/URL policyが利用不能なら回避せず、その段階を未実施として残す。この保存結果を新しい実行の成功証拠へ流用しない。終了後は所有profile・tab・serverを片付ける。

原本の場所は `provenance.json` の `<LOCAL_PHASE>` で抽象化した。raw20batch、catalog、HTML35件、server、initial3件はbyte同一でcopyMapに両hashを保持する。`failures.json`だけはprivate UI/操作argumentsを除き、local app pathをplaceholderへ変換した診断copyで、原本bytes/SHAと変換内容を別記する。全session transcriptや無関係な画面は公開しない。
