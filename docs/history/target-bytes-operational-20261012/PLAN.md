# 容量探索のupload/build運用評価計画 (#887)

対象main: `1962f233299aa10d6053230c1af1605faeaddc5d`。比較source: `21a15bc99d4c5258f640054e00e9958994e124a3`。
後者もJPEG初期化修正済みで、encoder/lock/compiler自体は同じ。探索と累積metricsの差を含むため、
独立した外側の経過時間を比較基準とし、旧totalMsを正しい総時間として扱わない。
公開npm 1.4.3の比較ではなく、同Mac/Nodeとlockを使う隔離clean checkoutのnativeを測る。

## 初期matrix

| ID群 | 入力と出力 | 入口・policy | 条件数 |
|---|---|---|---:|
| photo-* | CC0 coffee写真 → JPEG320px | compileImage、placeholderなし | 4 |
| ui-* | MIT synthetic ui-1 → WebP320px | compileImage、placeholderなし | 4 |
| large-* | 既存5000×5000 JPEG size fixture → JPEG1600px | compileImage、placeholderなし | 4 |
| cli-build | 同大きめ入力 → JPEG/WebP、320/1280px＋placeholder | 既存CLI、予算なし | 1 |
| cli-ui-tight | ui-1 → WebP320px | CLI、ui-tightと同policy | 1 |
| avif-none/tight | coffee → AVIF320px | compileImage、placeholderなし | 2 |
| api-range-tight | 大きめ入力 → JPEG1600px | public-upload付きtoFileTargetBytes、min40/max90/strict | 1 |
| api-known-max | 既存#885の64px RGB単色fixture → JPEG | public-upload付きtoFileTargetBytes、min1/max100/strict、346 B | 1 |

photo/ui/largeの4条件は、予算なし・余裕・q60単独encodeでbyte適合を示した厳しい予算・1 B不可能予算。
余裕は主測定前のq100 bytes×1.1（切上げ、q60 bytes+1以上）、厳しい予算はq60実bytesで固定する。
各workloadのq100/q60（計8回）と既知64pxのq50/q100と346 B strict探索を両sourceで確認（計6回）の限定calibrationを行う。
q60 byte適合は累積5秒で探索を完了できる保証ではない。その有効入力のtimeoutを今回観測する。
calibration失敗やq100/q60の非単調性は隠さず、主matrixの開始前に数値と条件JSON/hashを凍結する。

18条件×3trial×2source＝108主操作。sourceを条件/trialごとに交互に測り、全てfresh process・concurrency1。
観測は約10〜20分を見込む（build/setupは別）。既存helperの`isolatedCase`/`distribution`とcorpus検証を使い、
汎用benchmark基盤・新依存・製品algorithm/timeout/CI変更は作らない。

## 測定と失敗の境界

- API外側時間はsource constructorから処理完了/拒否まで。compilerはpreflight→処理→検証→local commitまで。
  source/module load後に開始し、worker起動は含めない。CLIはNode起動から終了までで別境界。
- compiler/APIは10ms sampled whole-process RSS。CLIはその子PIDだけの100ms sampled RSS。
  sampled peakは真のpeak、native heap、画像固有allocation、OOM耐性やconcurrency scalingの証明ではない。
- native metricsは公開APIで利用可能な値だけを保持する。compilerは公開メソッドを委譲する観測tapで取れる値を明示。
  CLIのpublic manifestにはnative metricsがないのでnull/未公開とし、preloadや製品改変で作らない。
- E300 byte不適合、E123 resource timeout、他Error、measurement failureを分け、timeoutした有効入力を期待 hostile拒否や受入へ加算しない。
- 成功時のmanifest/出力bytes/hash/quality/budgetを記録。source、native、lock/tool、fixture/license/runner、実command/exitを結ぶ。
  未観測のencode回数はnull。static上限とruntime観測を区別する。
- failed strict/compilerで新output未公開・owned output parent内のsibling staging残存なしを確認する。
  private source snapshotのOS tmp残存を独立に全走査せず、公開cleanupErrorの観測境界として記録する。既存compiler output保護と低レベルstrict file保護は
  代表conditionのpostcheckとして別時間/別結果に記録し、主trialの時間へ混ぜない。
- 旧known-maxのE300は既知q100が適合可能な誤拒否であり、成功の性能改善として比較しない。
  他の旧budget fitも、同じ最高qualityが証明されなければ結果差を明記し、同等成功と断定しない。

## 判断・納品

3trial/単一環境からp99・普遍的SLA・全codec優位・利用者採用を主張しない。p90はnearest-rankで3sample中最大。
compilerに存在しないquality-range/timeout policyを対策として案内しない。
測定からresource拒否と正しさの両方を評価し、SEMVER_POLICYのbug-fix例外と互換性リスクを根拠付きで分離する。
versionを先に決めず、「公開準備へ進める」か「この具体的な問題を先に解消する」を明記する。
後者でも本Goalは評価・候補/受入条件までで止め、製品実装や次Issueへ進まない。

専門レビュー1件（resource境界・公開判断）、セルフレビュー、関連修正と最終確認、Companion Gate、対象検証、
対応PRの必要CI→保護merge→merge SHAのCI/Security→#887完了報告。#703はOPEN、release/branch削除は行わない。

## Source/binary対応の修正

最初の共有Cargo targetによるsetupでは異なるsourceのnativeが同一hashになり、旧sourceの再compileも記録されなかった。
最初の108主記録・calibration・集計はinvalid-shared-target-cacheとして保持し、採用対象から除外する。
独立cacheの初回は既存Xcode clangとCommandLineTools SDKの不一致でjemalloc configureが失敗し、setup failureを保持した。
既存Xcodeのclang/SDK26.5を各buildコマンド内だけで選び、各clean checkoutの独立targetでcanonical buildする。
依存/CI/製品/グローバル設定は変更しない。採用するbuild receiptのsource/exit/native hashを各workerで照合し、
旧q50=472 B/q100=346 Bと旧strict誤拒否、新strict q100受入のsemantic対照も主測定前に確認する。
この具体的なprovenance疑問のためcalibration/対照は14操作へ増え、主matrix18条件/108操作は維持する。
