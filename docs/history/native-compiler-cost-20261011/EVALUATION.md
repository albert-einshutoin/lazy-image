# Native compilerの品質・容量・処理コスト評価

対象は[#883](https://github.com/albert-einshutoin/lazy-image/issues/883)、#703の子Issue。
**次の改善候補はJPEG normal-modeの設定初期化順序の修正検証1件。backend置換・新profileへの投資はその結果まで保留する。**
このPRは評価記録のみで、製品コードの修正・次Issue作成・#769/#881の再開は行わない。

## 出所と現在への適用

[Benchmark Regression run 37293485921](https://github.com/albert-einshutoin/lazy-image/actions/runs/37293485921)、
job [111709169333](https://github.com/albert-einshutoin/lazy-image/actions/runs/37293485921/job/111709169333)、
artifact `benchmark-regression` ID `11338520322`（取得時expired=false、期限2027-01-03）。
測定sourceは`37777bbc0b4988e888390e72a9093f602b8908ea`、**checkout/native 1.4.2**。
Ubuntu 24.04.5 x64、Node22.23.3、Rust1.88.0、available parallelism=4。
sharp実行版0.35.4 / libvips8.18.6 / libwebp1.6.0 / aom3.15.0、SSIM helper依存はlock上3.5.0。
native側lockはmozjpeg0.10.13/mozjpeg-sys2.2.3、webp0.3.1、libwebp-sys0.9.6、
libavif-sys0.17.0+libavif1.0.4、rav1e0.7.1、FIR6.1.0、image0.25.10、oxipng10.1.1、libc0.2.189。

[provenance](provenance.json)・[run/job](run-metadata.json)・[artifact metadata](artifact-metadata.json)に
各版・hash・policy・適用範囲を保存。compiler/quality JSONのhashはrelease集約JSONとも一致する。
corpus15件の入力とMIT/CC0 licenseは測定Git sourceと現在のbytes/hashに一致し、出所を保持した。
native bindingの実SHA、CPU型番、候補画像/生成物のpixelsは元artifactに保存されておらずunknown。
compiler version/fingerprint、jobのsource build成功、lock情報を、未保存のbinary digestへ読み替えない。
`release-evidence.md`の「v1.3.0」はrunnerの固定表題で、対象版の根拠には使わない。

比較先mainは`9e73bd07e3c99853f1f60000f71d811f3ef04438`（1.4.3）。src/lib/runner/helper/corpusは
Git treeと個別hashが一致。Cargo差分は本体版文字列、npmは本体/platform/Wasm版同期と
optional platformのlibc selector metadata削除で、外部依存の版/integrity変更はない。
これにより共有経路の診断と改善優先度には使えるが、**公開npm1.4.3の時間・品質・RSS実測ではない**。
再測定は行わない。新しいsource/依存・利用条件で絶対値を決める際には別記録で再測定する。

## A: compiler全体

正常受入 **13/13 case（39/39 trial）**、strict budget **11/11 case**、期待拒否 **2/2 case（6/6 trial）**。
処理/検証失敗0、保存された45 trialに測定不能0。期待拒否を正常受入に加算していない。
全caseのpolicy・時間分布・RSS・各trialの検査は[機械可読集計](case-summary.json)と[case表](case-tables.md)にある。

- 各正常trialでcommit後manifest・source checksum・artifact checksum・private metadata除去を確認。
  ICCを期待するmetadata-1/2は、各出力ICCと元ICCの一致もrunnerが検査する。
- 透過AVIFは元のregression fixtureでalpha最大差0、alpha-1で0、alpha-2で1/255（許容8/255）。
  これは該当AVIFの透過検査で、全codecのlossless alpha保証ではない。
- 期待拒否はhostile-1のplanning/E400（幅guard）、hostile-2のpreflight/E123（40MP guard）。
  どちらも公開directoryなし。無効入力を成功へ扱うfallbackはない。
- 正常caseのE2E medianは19.667〜966.965ms、最大trial985.077ms、sampled RSS最大121.41MiB。
  各caseは形式/幅/予算が異なるので、この範囲をformat別の速度として配分しない。
  例: coffeeはJPEG/WebP/AVIFの160pxセットでmedian966.965ms、jpeg-verification-ioは
  JPEG320/1057pxで246.069ms。whole-set bytes/digestは未保存で、予算対象のbytes等だけが残る。

時間/RSS窓はcompileImage呼び出しから検証・commit完了で返るまで。後段の外部Sharp監査や
HTML/配信は含まない。10msのJS samplingは一時peakやevent-loop停滞中のpeakを捕捉できない。
3 trialのnearest-rank p90は各caseの最大値であり、利用者SLAや母集団p90ではない。

## B: codec比較

正常入力13件×JPEG/WebP/AVIF=39比較。共通referenceはauto-orient、最大幅320px、白背景合成PNG。
sharp q80（JPEG/WebP）/q60（AVIF）のSSIM/bytesを共通下限/上限にし、12 quality候補を各3回encode。
同じq値を同じ画質とは扱わない。二つの選択は独立で、両方を同時に満たす候補の探索ではない。

| 形式 | lazy SSIM下限達成 | lazy byte上限達成 | 下限達成時に小さい / eligible | 下限達成時にencode medianが速い / eligible | lazy/sharp容量比範囲 |
|---|---:|---:|---:|---:|---:|
| JPEG | 1/13 | 12/13 | 0/1 | 0/1 | 1.037 |
| WebP | 13/13 | 13/13 | 5/13 | 0/13 | 0.945〜1.246 |
| AVIF | 11/13 | 12/13 | 0/11 | 7/11 | 1.147〜3.546 |

sharpは両条件39/39達成。lazyの品質未達14件・byte未達2件は、処理失敗/測定不能とは別である。
保存39比較では全候補に3時間sample・SSIM/PSNRがあり、処理失敗/測定不能は0。
同じbyte cap内の最良SSIMがsharp以上だったのはJPEG0/12、WebP6/13、AVIF0/12。
WebPの条件付きencode median比は1.18〜3.96倍、AVIFは0.20〜1.23倍で、compiler E2E比較には使わない。
grid-worker RSSはlazyが各形式13/13でsharpより高かった。lazy範囲はJPEG106.17〜176.12MiB、
WebP105.22〜179.20MiB、AVIF122.46〜206.62MiB。allocationや一般的なOOM耐性の結論にはしない。

| 入力種別 | 分母/形式 | JPEG 品質/容量達成 | WebP 品質/容量達成 | AVIF 品質/容量達成 |
|---|---:|---:|---:|---:|
| photo | 3 | 0/3・3/3 | 3/3・3/3 | 3/3・3/3 |
| UI | 2 | 0/2・2/2 | 2/2・2/2 | 2/2・2/2 |
| illustration | 2 | 0/2・2/2 | 2/2・2/2 | 2/2・2/2 |
| alpha起点（白合成） | 3 | 1/3・2/3 | 3/3・3/3 | 1/3・2/3 |
| metadata起点 | 3 | 0/3・3/3 | 3/3・3/3 | 3/3・3/3 |

代表未達はcoffee/JPEG（最高SSIM0.928983 < floor0.940866）、UI-1/JPEG（0.987631 <0.989706）。
alpha-1/AVIFは0.995753 <0.995883、alpha-2/AVIFは0.998719 <0.998938。
transparent-avifの最小JPEG337 B > cap326 B、AVIF327 B >288 B。JPEG/AVIFのbyte未達はこの1入力だけ。
未達はこの有限grid/設定で候補がない意味で、codecの原理的限界ではない。
最小候補は最初に条件を満たすqではなく最小bytesで選ぶ。例えばtiny AVIFはq100の327 Bが
q10の328 Bより小さく、UI-2/AVIFはq85の5,833 Bが条件を満たすq80の6,213 Bより小さい。
78 engine比較の選択を全候補と照合し、不一致0。集計scriptも条件充足・最小性/最大SSIMを検証する。
tiny fixtureでSSIM=1でもPSNRが有限な候補があり、pixel一致や人間の知覚同一性を意味しない。

quality時間は各encode呼び出しだけで、reference作成/quality計算は時間外。
RSSはreference作成後からgrid全体をsampleし、両workerとも両libraryをloadする。
referenceの残存memory・品質decode等を含み、reference作成中の一時peakやencoder単体allocationは測れない。
白合成の「alpha起点JPEG」比較を、compilerのalpha→JPEG受入や透過保持へ転用しない。
元workflowの旧baseline比較にはwarn-modeの時間regression4件も残る（[原集約](raw/regression-summary.json)）。
これは異なる旧baseline条件で、compiler/制約付き品質の時間順位ではない。

## 次の改善1件と投資判断

**JPEG normal-modeの初期化順序を検証・修正する候補を最優先にする。**
12/13品質未達をすぐ[#638 jpegli置換](https://github.com/albert-einshutoin/lazy-image/issues/638)の根拠にしない。
coffee等は12 quality要求に対し出力hashが4種類で、q10〜50、70/80/85、90/95/100が各plateau。
共有経路では`set_quality`後の`set_scan_optimization_mode`が`jpeg_set_defaults()`を呼び、
default quality75へquantizationを再初期化する。
[固定crate checksum/source抜粋](jpeg-setting-source.json)と現encoder sourceにより設定順序の問題を確認した。
artifactには実行時quantization tableのdumpがなく、保存JPEG plateauがこの経路と整合するという診断を越えて
新しいruntime結果を捏造しない。smoothingのband差が一部bytes/SSIM差を生むこととも整合する。

次の作業はcodec置換ではなく、defaultsをresetする設定と最終quality/chroma等の適用順の限定修正検証。
対象はcoffee、UI-1、metadata-1とJPEG normal/fast-modeの共有経路。
期待する効果は明示qualityがquantizationへ反映され、JPEG探索が有効なquality候補を扱えること。
SSIM下限達成率の改善量や容量/時間の優位は未確定で、結果が悪ければ採用しない。
公開API・既定quality数値、progressive/chroma、ICC/private metadata、strict budget失敗時非公開、
panic/error境界を維持する。出力bytesの変更は明示qualityの意味を戻す変更として別途レビューする。
同じsmoothing bandのq90/100等でDQT/decoded qualityを検査する最小回帰チェックを先に置き、
既存targeted testsの後、正本`npm run build`→`npm run test:bench:release`を別記録で実行する。
corpus・閾値・gridは変更せず、旧結果を上書きしない。

[#640 WebP profile](https://github.com/albert-einshutoin/lazy-image/issues/640)は、13/13達成の一方で
容量の勝敗が混在し時間/RSS優位はないため、自動着手しない。
[#641 PNG](https://github.com/albert-einshutoin/lazy-image/issues/641)はPNG出力/profileがこの比較対象外で判断保留。
backend/profile投資の再開には利用者自身の入力、許容画質/透明度/ICC、実幅、byte上限と処理時間/memory条件が必要。
この評価からSLAや許容値を設定しない。

WebPを検証済みartifact準備の出発点とし、hard capはstrict policyで失敗を扱い、画質は用途別に確認する。
JPEGはquality設定経路の確認が先、AVIFは相対的なencode時間と容量のtrade-offを用途で評価する。
静的サイト例の320/640/960pxは生成確認で、ここは最大320pxの制約付き品質評価。
640/960px品質、公開npm1.4.3の絶対時間/RSS、実利用者の採用/市場適合は未確認である。

## 再現・検証

```bash
node docs/history/native-compiler-cost-20261011/summarize.cjs --self-test
node docs/history/native-compiler-cost-20261011/summarize.cjs --check
```

保存JSONのhash照合・分母/未達分類・候補/参照整合・再生成一致のみを検証する。
画像処理/build/benchmarkの再実行、新指標・依存・baseline更新は行わない。
