# Compiler budget maxQuality受入評価 (#889)

## 契約

budgetのmaxQualityは30〜100の整数、省略100、下限30。100は省略と同じ正規形へ統一し、非既定値だけをbudget/plan/fingerprintへ含める。
共有target-bytes探索へ範囲を委譲し、範囲内の最高byte適合qualityを選ぶ。選択qualityの整数/範囲を検証してからmanifestを公開する。
strict、累積resource制限、privacy/ICC、非公開/既存出力保護、metrics、budgetなし/placeholder/低レベルAPIは維持する。

## 凍結する実測条件

同じApple M4/Node24.2.0、concurrency1、fresh worker process。既定policyと上限付きpolicyを同じ新JS実装で分け、各3trialを測る。
新source `574b81148b4f2fc30ddab567644e1037c2416343` の隔離clean checkoutを使用する。
nativeは同checkoutの独立target/locked CLI3.9.0/canonical npm run buildで生成。build source `464187c4a428f95263e9f6ef456f4bfb118fef01` からの差はcanonical生成index.d.tsのみで、runtime JS/Rust/lock差分がないことを照合する。

| workload | fixture/width/format/maxBytes | policy | interface/trials |
|---|---|---|---|
| large-tight | #887 large5000×5000→1600 JPEG / 93,782 B | 省略100・明示90 | compiler API/CLI、各3 |
| avif-tight | #887 CC0 coffee→320 AVIF / 12,850 B | 省略100・明示60 | compiler API/CLI、各3 |

8条件×3＝24主操作。CLIのcause/cleanupErrorは公開diagnosticsで未観測のままにし、E123だけからtimeout causeを推測しない。
限定回帰4操作を別集計: UI320 WebP/2,218 B/max60、coffee320 JPEG予算なし、非単調64px JPEG/346 B/max100受入とmax30 strict拒否。
別のhighest-quality対照35操作: large JPEG q90→q60（31）、AVIF q60、WebP q60、control q30/q100。
選択q60のbytes/hashが独立単独encodeと一致し、JPEGのq61〜90がすべてbyte未達であることを確認する。対照の時間/RSSを主操作へ混ぜない。

コマンド: `node docs/history/compiler-budget-max-quality-20261012/evaluate.cjs --freeze`、`--run`、`--proof`。
固定fixture/licenseは#887のmanifestを再利用し、元raw・無効batch・失敗は変更しない。概算3〜5分。

## 観測境界と安全性

API外側時間はmodule/fixture load後からcompileImage終了まで、CLIはNode起動からexitまで。API process RSS10ms、CLI child PID RSS100msのsamplingで、native metricsと別に記録する。
encode/probe実回数は未観測null。source・JS実装・native・fixture/license・runner/helper・command/exitを結ぶ。
全拒否で新output未公開/owned parent sibling stagingなしを確認。成功代表で既存manifest/全artifact保護を別時間に記録。
OS tmpのprivate source snapshot残存の独立全走査は行わず、公開cleanupErrorと対象テストの境界を明記する。
単一環境/3trialからSLA/p99/並列/OOM/知覚品質/公開npm同一実装を主張しない。maxQualityは時間や知覚品質の保証ではない。

## 検証・公開判断

policy/API/CLI/型、非単調/範囲外のみ適合時strict拒否、選択quality異常、ICC/privacy/metricsの対象検証と実測を行う。
セルフ＋専門レビュー1件、関連修正と指摘範囲の最終確認後に必要な完全検証を実施し、Companion Gateを最終差分に結ぶ。
opt-inの受入改善と既定policyのtimeoutを別に記録し、保留解除の判断は実測・残る制約・追加公開契約を根拠に行う。
SemVerは追加機能として評価し、version/tag/publish/release/deploy、別Issue、branch削除へ進まない。
