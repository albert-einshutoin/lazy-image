# 対象検証と最終検証記録 (#889)

## 変更前の失敗とsetup

開始main16662b40…に契約テストを追加し、`node test/integration/artifact-policy.test.js`がexit1、E400 unknown budget key maxQualityで失敗することを確認してから実装した。
最初のCLI対象検証は外部nativeをNAPI_RS_NATIVE_LIBRARY_PATHで指定し、API/JSONフロー後のpacked smokeでcheckout-local binding guardが失敗（exit1）。製品成功として扱っていない。
rootの既存nativeを上書きせず、隔離checkoutでcanonical npm ci/buildがexit0、CLI3.9.0を確認。index.d.tsは生成元を変更しcanonical buildの4行差分を取り込んだ。index.jsの生成差分はない。
source464187c…のbuildとsource574b811…の実行の差は生成型のみ。実runtime JS/Rust/lock一致はworkerで確認する。

## source574b811… / checkout-local nativeの対象検証

| command | exit | 範囲 |
|---|---:|---|
| `node test/integration/artifact-policy.test.js` | 0 | 省略/100互換golden digest、30/60/90/99/100、一意性、不正値/own-data/未知キー |
| `node test/integration/artifact-compiler.test.js` | 0 | 最高適合、上限外のみ適合時strict拒否、異常quality非公開、E123非公開、既存出力、cap付きICCと既存privacy検証 |
| `npm run test:cli` | 0 | API/CLI JSON一致、不正cap E400、非公開、signal、locked packed CLI/local native |
| `node test/integration/target-bytes-file-native.test.js` | 0 | 既存共有探索/非単調最高quality/atomic file/strict既存file保護 |
| `npm run test:types` | 0 | public numeric option/plan型と文字列拒否 |
| `npm run test:generated-artifact-policy` | 0 | canonical生成契約 |
| `node test/unit/target-bytes-search.test.js` | 0 | 既存shared helper/strict境界 |
| `node test/integration/basic.test.js` | 0 | 既存encoding/metrics/target-bytes defaults、50 passed |
| `node test/integration/quality-metrics.test.js` | 0 | 既存native品質metrics |

実測run/proofはexit0。`summarize.cjs --check`は24/4/35記録、保存binding、条件/source/environment、range/最高適合/既存出力を確認しexit0。
画像の実probe回数は推測しない。CLI E123 cause未観測はconfirmed timeoutと分離する。

## 納品時の確認

セルフレビューはpolicy→plan→共有探索→公開前quality/byte/metadata検証、default canonical互換性、生成元/型、独立highest-fitと記録境界を確認した。
公開policy/input/resource/公開準備の専門レビュー1件はSol medium、READ_ONLY/STATIC_ONLYで**ALLOW**、直接の受入・回帰・security/privacy・provenance指摘なし。実行検証やhosted成功を静的ALLOWから主張していない。
必要な完全検証、Companion Gate、最終HEAD/merge SHAのhosted証拠は納品時に追記する。
過去#887のGate/CIを今回の成功証拠として扱わない。完全検証/公開準備の判断はこのGoalの最終記録に結ぶ。
Companion初回GateはcompilerFingerprintへJS sourceを含むという説明の誤りでBLOCK。実関数は変更せず、JS実装をsource SHA/個別hashで評価へ拘束し、非既定capをpolicy identityへ反映する説明へ訂正した。製品コード/測定データの変更・再測定は行わない。

## 完全ローカル検証

公開policy/inputの高リスク変更であり、専門レビュー確定後に完全検証を行った。
canonical `ci:plan-tests` は公開型index.d.ts変更を理由にfull昇格。`ci:run-tests --mode full`がcanonical `npm test`を実行しexit0、failure0。
既存Wasm packageのregressionはnpm testに含まれるが、Wasm実装・新評価・公開は行っていない。
続く`npm run test:bench:release`もexit0。source574b811…、SDK26.5/CC/独立target、実command/時刻/exit/log hashは[receipt](validation/receipt.json)、[test-summary](validation/test-summary.json)を参照。
tracked checkoutは前後clean、native hashはc15f1e55…のまま。追加の全build/全benchmark再実行は行わない。

release evidenceのcompiler正常13/13、strict11/11、期待拒否2/2。期待拒否はE400不正dimensionとE123 pixel-limitであり、今回の有効入力timeoutと異なる。
codec品質37/39、byte37/39で、品質未達2/39・byte未達2/39を保持した。release-evidenceのpassedはその検証契約の成功であり、全codec target達成や実release承認を意味しない。
[compiler raw](validation/compile-image-corpus.json) / [quality raw](validation/quality-matched.json) / [evidence check](validation/release-evidence.json) を別に保存した。
