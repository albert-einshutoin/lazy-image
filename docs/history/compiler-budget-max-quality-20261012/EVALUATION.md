# Compiler budget maxQuality契約・受入・公開準備判断 (#889)

**判断: 明示maxQualityを使う運用policyを前提に、公開準備へ進める。**
#887の保留理由だった「同じ幅/byte予算を保ったcompiler受入の選択肢がない」問題に、既存resource制限・strict・検証済みmanifestを維持するopt-inの上限を提供できた。
既定policyのtimeoutは解消していない。上限省略で同じ受入を保証する判断でも、release実行・公開承認でもない。
新契約・代表条件の証拠・残る制約を納品し、必要な完全検証とレビューの成功をもってこのGoal内の公開準備判断を確定する。
専門レビューと完全ローカル検証は成功した。最終Companion Gate・hosted検証は納品時の最終HEADへ結び、実releaseは行わない。

## 契約と互換性

`policy.budgets[].maxQuality`は30〜100の整数、省略100、探索下限30。範囲内でbyte適合する最高qualityを既存の共有探索から選び、範囲外は候補にしない。
適合候補なしはstrict拒否、resource超過は従来のE123。timeout緩和/reset、途中候補成功、codec/backend/低レベルAPIの変更は行っていない。
budgetなしのJPEG85/WebP80/AVIF60とplaceholderは維持する。
文字列/小数/null/範囲外/accessor/継承・未知キーをE400として拒否し、coercion/clampを追加していない。

省略/明示100は同じ正規形にする。100を正規化budget/planから省き、既存policyのserialized shapeを保持する。
同じsource/compilerFingerprintでの旧policyFingerprint/cacheKeyを変更前の保存digestと照合し、30/60/90/99/100の異なる意味は区別した。
compilerFingerprint自体はJS sourceを含まない。今回のJS実装は評価のsource SHAと個別JS hashで拘束する。
非既定maxQualityはpolicyFingerprint/cacheKeyへ反映し、省略/明示100は意図どおり従来identityを維持する。
manifestは実際の選択qualityを記録し、整数30..maxQualityとbyte上限の検証後に公開する。schemaVersionの並行分岐は追加しない。

[SEMVER_POLICY](../../SEMVER_POLICY.md)の「安全な既定値を持つoption追加」に当たるMINOR相当の追加機能。
省略時の探索/出力/拒否とfirewall値は維持する。version番号・tag/npm publish/release/deployは行わない。
`maxQuality: 60`は知覚品質60%、lossless、時間保証の意味ではない。compilerのminQuality/effort/attempt/timeout overrideは存在しない。

## source・fixture・測定境界

runtime sourceは`574b81148b4f2fc30ddab567644e1037c2416343`、同じM4/10 CPU/32 GiB、Darwin arm64、Node24.2.0。
隔離clean checkoutでlock対応npm ci、CLI3.9.0、独立targetのcanonical npm run buildがexit0。
build source `464187c4a428f95263e9f6ef456f4bfb118fef01` からの差はcanonical生成index.d.tsのみで、runtime JS/Rust/lock/build inputsの差がないことを各workerで照合した。
native SHA256は`c15f1e55f5552ec49d4b853bb49a3eed26141b071b7772c166eadeab8649d841`。
Rust/helpers/依存は開始base `16662b40a922d4fc47a65c93d08ed76deb06ad2a` と同じ。JSだけの契約追加であり、native hashの旧#887との違いだけを性能修正や誤対応の証明にしない。
[setup](evidence/setup.json)、matrix/各rawのsource/JS/native/lock/fixture/license/runner/helper/command/exitを正本とする。

[PLAN](PLAN.md)の8条件×各3trial＝24主操作、concurrency1、fresh worker。各trialで同workloadの既定/上限policyを交互に測定した。
API外側はmodule/fixture load後のcompileImage開始から終了まで。CLIはNode起動からexitまで。
API whole-process RSS10ms、CLI child PID RSS100msのsamplingは別境界で、native peakRss/CPUはprocess-wide。
出力hash検証、既存出力保護、独立quality対照の時間/RSSは主測定へ混ぜない。実encode/probe回数は未観測null。

## 主結果

[RESULTS](RESULTS.md)、[24 raw](evidence/raw/)、[集計](evidence/summary.json)を参照。

| workload | 既定API | 上限付きAPI | 上限付きCLI | 選択quality/bytes |
|---|---|---|---|---|
| 5000×5000→JPEG1600、93,782 B、max90 | 3/3 E123 timeout、p50 5,550.2 ms | 3/3受入、p50 3,347.9 ms、worst4,025.9 ms | 3/3受入、p50 3,204.4 ms、worst3,222.2 ms | q60 / 93,782 B |
| coffee→AVIF320、12,850 B、max60 | 3/3 E123 timeout、p50 5,061.1 ms | 3/3受入、p50 97.2 ms、worst97.9 ms | 3/3受入、p50 129.2 ms、worst133.8 ms | q60 / 12,850 B |

上限付きはAPI/CLI合計12/12受入。省略policyはAPI6件がcause付きtimeout、CLI6件がE123 resource拒否でcauseは未観測。
CLIの時間が近いだけでtimeout causeを実証したとはしない。既定の拒否を上限付きの成功で上書きせず、有効入力timeoutを期待 hostile拒否へ加算しない。
JPEG API sampled RSSは既定276.7–276.8/上限276.5–276.8 MiB。AVIFは96.4–96.5/93.4–93.8 MiB。
CLI/API間のRSSや時間を直接順位にせず、JPEG memory低下や普遍的な省メモリを主張しない。

JPEG上限付きのnative totalMsは2,681–3,493 ms、選択候補encodeMsは58–88 ms。AVIFは78 ms前後。
選択候補のencode時間と累積探索時間は別で、回数をquality差から埋めない。CPUはprocess-wide seconds、peakRssはprocess lifetime高水位でsampling peakと同じ値ではない。
現行5秒はcodec強制停止ではなく処理境界の検査。外側5秒以内、p99/SLA、並列scaling、OOM、知覚品質、全画像/環境の受入は保証しない。

## 最高適合と安全性の独立確認

[35 single-quality対照](evidence/proof/)は別時間。JPEG q61〜90は全30候補が93,782 B超過、q60は予算適合。
AVIFは上限q60が適合する。各q60のbytes/hashがAPI/CLI全成功trialと一致した。
JPEG SHA256 `283dcdcabaa25ce5302f33c700e475f2009786779c018d930c01eb67955af857`、AVIF `fc3908d6154f0c0315d2ac917361251f0810bbef2638c8cd783e23bd1e605da7`。
これは指定範囲内の最高適合であり、上限外の品質・SSIMを評価した結論ではない。

限定回帰4操作はWebP max60受入、予算なしJPEG q85、非単調64pxのq100/346 B受入、max30の同budget E300。
上限外q100だけがbyte適合する対照を、max30で成功にfallbackしていない。
全拒否で新output非公開/owned parent sibling stagingなし。成功代表への再compileはE301、manifest/全artifactのhashを保持した。
private source snapshotのOS tmp残存は独立全走査していない。公開cleanupError、対象テストの失敗時cleanup検証、owned parentの範囲を区別する。
ICC/private metadata・累積metricsの関連回帰は既存方式の対象検証と完全検証へ結ぶ。[VALIDATION](VALIDATION.md)参照。
完全検証の既存corpusは正常13/13、strict11/11、期待拒否2/2。codec品質/byteは各37/39で、各2件の未達を保持した。これを全画像の品質・容量優位の証拠としない。

## 公開準備へ進める理由と残る制約

上限を明示すれば同じ幅/byte予算でverified compiler/CLIの受入を回復でき、品質範囲を利用者のpolicyへ表せる。
strict・最高適合・privacy・累積resource/metricsを維持しており、低レベルAPIへ迂回せずmanifestを得られることが#887からの変化である。
従来の省略policyは最高100までの探索を続け、上記timeoutが残る。これを既定policyの修正完了とはしない。
運用説明では上限の選択によるquality trade-off、拒否の扱い、単一環境3trialの境界を明示する。導入先のjob条件で別途受入を確認する。
今回の公開準備判断はこの明示policy契約を前提とする。汎用的な省略policy受入や実releaseの可否へ拡大しない。
本Goalは#889の実装・証拠・検証・PR納品までで停止し、releaseや次Issueには進まない。
