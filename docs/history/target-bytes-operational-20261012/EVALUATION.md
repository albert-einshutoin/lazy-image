# 容量探索のupload/build受入・コスト・公開準備判断 (#887)

**判断: 公開準備へ進めず、compilerの固定探索と累積5秒policyの組合せで、byte適合可能な代表条件がtimeoutする受入問題を先に解消する。**
本Issueは評価・運用説明・候補と受入条件の記録で完了し、製品修正・別Issue・releaseには進まない。

## 対象と固定条件

main `1962f233299aa10d6053230c1af1605faeaddc5d` と、JPEG初期化修正済み・探索変更前
`21a15bc99d4c5258f640054e00e9958994e124a3` を同じApple M4/10 CPU/32 GiB、Darwin arm64、Node24.2.0で比較した。
両lock・encoder・compilerコードは同じ。差には容量探索と累積metrics修正があり、独立した外側時間を基準とする。
package/nativeの1.4.3表記は公開npm同一実装の証明ではない。今回の測定はcheckout nativeだけである。

各clean checkoutでlock対応のnpm ci、CLI3.9.0、canonical npm run buildを使用。既存Xcode clang/SDK26.5を
buildごとに選び、独立targetで生成したnativeはmain `34926526c54f42cab8151250d648550fa7062af61c57f20d79e64b66e5578223`、
旧 `1926c6c8157af958f12a154afa5829684afa5802627e768fe7c538a15a687670`。
source/command/時刻/exit/SDK/nativeは[evidence/matched-sdk-build-results.json](evidence/matched-sdk-build-results.json)、
各workerもclean HEAD・local addon load・hashを照合した。

[PLAN](PLAN.md)の18条件×各3trial×2source＝108主操作、concurrency1、fresh processでsourceを交互に実行。
写真はCC0 coffee、UIと5000px size fixtureはsynthetic/repository fixture、64pxは#885の固定RGB対照。
fixture/licenseのpath/bytes/hashと由来は[fixtures.json](fixtures.json)。大きめfixtureの過去の生成実行自体は証明しない。
[凍結matrix](evidence/matrix.json)のSHA256は`63943e56dcc5d73c9b818ae8ef06b68b63154d792b3a06183eed813f2cbe963d`。
予算はq100 bytes×1.1の余裕、q60実bytesの厳しい値、1 B不可能値を主測定前に固定。
14回の限定calibration/semantic対照と、6回の付帯保護/best-effortチェックは主時間と別に記録した。

## 受入結果と主要なコスト

[全18条件の結果](RESULTS.md)、[集計](evidence/summary.json)、[108 raw](evidence/raw/)を正本とする。

| 主操作の結果 | 旧 | main |
|---|---:|---:|
| 受入・budget条件適合 | 42/54 | 39/54 |
| E300予算不適合 | 9/54 | 6/54 |
| 確認できたE123 timeout | 0/54 | 9/54 |
| 既知q100の誤拒否 | 3/54 | 0/54 |
| 他Error/measurement/safety failure | 0 | 0 |

1 B不可能予算9操作を除いたbyte適合可能条件45操作では、main39/45受入、6/45 timeout。
compiler/CLIに限れば、byte適合可能条件39操作で旧39/39、main33/39受入（6 timeout）。
これは全入力の受入率・実利用者の成功率ではなく、固定matrixの分母である。
既存13/13 corpus受入とこの新matrixは幅/予算が異なるため、同じ評価として置き換えない。

- 大きめ5000×5000入力→JPEG1600px/93,782 B: q60単独はbyte適合、旧3/3受入・外側p50 1,017.7 ms。
  mainは3/3 E123、p50 5,559.4 ms、worst5,566.4 msで未公開。
- coffee→AVIF320px/12,850 B: q60単独はbyte適合、旧3/3受入・477.7 ms。
  mainは3/3 E123、5,057.4 ms、worst5,094.7 ms。
- 同大きめJPEGでImageEngineの既存min40/max90を明示: main3/3受入・2,792.7 ms、worst2,821.6 ms。
  これは低レベルAPIの選択であり、compiler policyに同じ設定は存在しない。
- 写真320pxのtight8,415 Bは3/3受入を維持し、p50 51.2→355.0 ms。UI320pxのtight2,218 Bは85.8→351.5 ms。
- 余裕予算は両sourceともq100で受入。写真p50 136.9→62.4 ms、UI55.5→18.0 ms、大きめ3,138.1→1,637.7 ms。
  これらの改善をタイト/不可能予算や全画像へ一般化しない。
- 予算なし複数出力CLI（JPEG/WebP320/1280＋placeholder）は3/3受入。p50旧1,672.5/main1,705.8 ms。
- 64px/346 B対照は旧3/3 E300（既知q100が適合可能）、main3/3 q100受入。
  旧7.2 msの速い誤拒否をmain10.8 msの正しい成功と同等に扱わない。

両sourceで成功した条件はquality/bytes/hashも比較する。同じ数字qはcodec間の知覚等価ではなく、
旧budget fitだけで全候補の最高qualityが証明されたとはしない。この評価ではSSIMを再測定していない。

## 原因として言える範囲・memoryとtimeout

大きめJPEG単独calibrationのq100は外側855.3 ms/encode722.7 ms、q60は187.6/62.1 ms。
decode約97–99 ms、resize約17 msが観測された。AVIF単独q100は259.0/encode246.0 ms、q60は92.4/79.7 ms。
高quality自体のコストと、探索・compiler全体は分けて扱う。失敗trialには選択候補metricsがなく、
実encode/probe回数を観測していないためnullのまま。quality差から回数を推測しない。
成功したbudget APIのencodeMsは選択候補の情報で、全probe時間への読み替えや合計による原因推定はしない。

timeout rawのcauseはencode境界で5000ms超過を示す。codec中の強制停止ではないので外側は5秒を超え得る。
同じ5秒を単独q60が通っても、先行probeを含む探索全体の完了を保証しない。timer reset/緩和/途中候補成功は使わない。

大きめAPI/compilerのsampled whole-process RSSは旧/mainとも概ね268–277 MiB。
AVIF tightは旧94.1–94.4/main96.1–96.5 MiBだった。本条件では時間/受入の差が主で、memory低下を主張しない。
APIは10ms、CLIは子PIDだけ100msのsamplingで、CLI/API間の数字を直接比較しない。
native peakRssはprocess lifetime高水位（利用不能時はestimate）で、sampled peakとは別。
reference/module/fixture準備後に外側計測を始め、出力hash検証・付帯checkは外側時間/RSS windowに入れない。
単一環境・3trial・concurrency1からp99、普遍的SLA、OOM耐性、並列scalingを主張しない。

## 安全性と運用対策

全E300/E123で新しいoutputとsibling stagingを公開していないことを確認した。
付帯チェックは各sourceでcompiler既存manifest/全artifact保持（E301）、strict既存file保持（E300）、
64px best-effort不適合を記録。best-effortのbudgetMet=falseを受入へ加算しない。
private source-snapshotのOS tmp残存を独立に全走査した証明ではない。公開cleanupErrorとowned output parentの観測範囲を守る。

compilerで可能なのはwidths/formats/budgets/placeholderの選択。予算なしや余裕条件は今回受入したが、
byte上限を満たさなくてよいという意味ではない。アプリはbyte policyの変更を明示判断する。
ImageEngineはmin/maxQualityを選べるが、検証済み画像一式・manifestを返すcompilerの代替と同一ではない。
E300は必要な品質・幅・formatと予算を再検討。E123は有効入力でもresource拒否として扱い、無条件retryせず、
幅/format/出力数やjob設計を見直す。compilerに任意quality範囲やtimeout設定を案内しない。

## SemVerと次の1段階

[SEMVER_POLICY](../../SEMVER_POLICY.md)の文書化された意図へ揃えるbug-fix例外に、JPEG quality反映、
最高適合quality、累積metricsの修正は該当する。API形状・既定数値・metadata・firewall値は変わっていない。
ただし同入力/policyで受入からE123へ変わった代表条件は、公開判断で扱うべき実際の互換性/運用リスク。
bug fixという分類だけで公開可や特定のversion番号を決めない。

次の1段階は、このcompiler受入問題の設計・修正評価。候補は、最高quality/effortの明示的な上限を扱う契約設計、
または最高適合を保った処理コスト削減である。前者は現compilerにない機能で、別の公開契約レビューが必要。
このGoalでは実装・新Issue・versionを作らない。
受入条件は、上記1600px JPEGと320px AVIFのbyte適合条件で既存resource制限を維持し、最高適合の正しさ、
累積metrics、ICC/private metadata、失敗時非公開/既存出力保護を保って、named環境で受入を示すこと。
誤った旧探索・timeout緩和・途中候補による成功に戻さず、新しい独立した時間/source/native証拠で判断する。

## 保持した無効/失敗と証拠の再確認

最初の共有Cargo cache batchは異なるsourceで同一native hashになり、sourceとの対応が不成立。
[evidence/invalid-shared-target-cache](evidence/invalid-shared-target-cache/)の108記録/集計/ログをそのまま保持し、
上の数字には使わない。独立cache初回のSDK mismatchによるbuild exit1もsetup failureとして保持。
build失敗log2件はローカル原本を保持し、公開用コピーから利用者固有pathを除去した派生物である。[LOG_REDACTIONS](LOG_REDACTIONS.json)が元/公開bytes・hashを結び、公開保存hashは派生物を対象とする。
その後のbuild/source/native対照と有効108記録だけを採用する。過去Linux/native1.4.2や旧GoalのGate/CIは今回の成功証拠ではない。
