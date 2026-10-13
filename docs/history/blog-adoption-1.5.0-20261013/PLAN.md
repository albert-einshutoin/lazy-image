# 公開1.5.0 技術ブログ6画像の固定評価計画 (#902)

対象は公開npm Native compileImage 1.5.0、source `023a8c9368eef58f17ecc6f412363d24e5aa9b67`。評価base `39e348bed04daa7072e83a80a351f5ee268d3575`。実利用者の採用実績や全入力/全OSの保証ではなく、1環境で既存素材による用途再現を行う。実装・依存・CI・releaseは変更しない。

## 入力・policyの事前固定

[inputs.json](consumer/inputs.json)は6点のsource/license/実bytes/hash/寸法と、A/Bの具体的なpolicy JSONを指す。[release corpus](../../../test/benchmarks/corpus/release-manifest.json)の写真2点とui-1/metadata-1のhash・bytesを実ファイルと照合した。coffee/chelseaはCC0写真、ui/metadataはproject-generated MIT合成素材、大入力はrepository MIT合成size fixture（当時のgeneratorの詳細は未保存）、coffee-64は公開ImageEngine resize→PNGの派生素材。派生を別写真とは数えない。素材を結果に応じて入れ替えない。

| 入力 | encoded dimensions | display幅 / 実出力幅 |
|---|---|---|
| coffee | 600×400 PNG | 600 / 320 |
| chelsea | 451×300 PNG | 451 / 320 |
| ui-1 | 320×240 PNG | 320 / 320 |
| metadata-1 | 320×240 JPEG、Orientation6 | 240 / 240 |
| large | 5000×5000 JPEG、3,371,242 B | 5000 / 320,640,960 |
| coffee-64 | 64×43 PNG | 64 / 64 |

公開inspectのbounded scanでorientationKnown=falseとなるcoffee/chelsea/largeは、その観測をそのまま保存した。consumerがtrueへcoerceせず、compileImageの既存完全container検査・preflightへ渡す。

A/Bとも要求幅320/640/960、WebP/JPEG、placeholderあり。Aはbudgetsなし（既定WebP80/JPEG85）。Bは以下の評価目標。1 KiB=1024 B。maxQualityを知覚品質の百分率とは扱わない。

| 幅枠 | WebP maxBytes / maxQuality | JPEG maxBytes / maxQuality |
|---|---|---|
| 320 | 40 KiB / 80 | 60 KiB / 85 |
| 640 | 100 KiB / 80 | 150 KiB / 85 |
| 960 | 200 KiB / 80 | 300 KiB / 85 |

budgetsは正規化で残る実幅だけに指定する。全要求幅が大きいmetadata/派生小画像では、実幅240/64に320枠の同じ予算と上限を指定し、削除される640/960へbudgetを置かない。これは事前に固定した小画像の適用ルールであり、失敗後の緩和ではない。[具体policy](consumer/policies/)を正本とする。

## 測定・受入・失敗

`EVALUATION_REVISION=<freeze commit> node measure.mjs <new-runs-directory>`。A1,B1,A2,B2,A3,B3順の6 process、各6画像逐次、計36 compile。code hash、実command、exit/signal/error、個別compile外側wall、module load後のpage wall、Node起動/終了込みprocess wall、実manifest、全生成fileのbytes/hashを保存する。初回も残し、各3値/中央値/最大値を報告する。追加の並列化/cacheなし。準備・出力監査・browser/品質観察は測定時間外。

公開compileImage manifestにnative metricsは露出しないためnative totalMsはnull/未取得。外側時間からencode回数やnative内訳を推測しない。製品のerror phase/code/cause/cleanupErrorはそのまま保存し、有効入力のE123を期待拒否として加算しない。評価codeの不具合があれば旧測定を無効理由付きで保持し、変更hashと最終測定を分ける。

B目標：18/18正常受入、全budget適合、pageの各process外側60秒以内、表示/品質/負例保護。未達を予算・入力・resource緩和で元の成功へ置き換えない。60秒は単一M4環境の用途基準で、p95/p99/SLAではない。

HTMLは6画像の全compile成功後に書く。失敗画像があっても残りの固定入力の観測を続け、成功ページは作らない。既に確定した画像は残り得る。画像単位のcompiler確定をサイト全体atomic更新とは呼ばない。新規siteを要求し、既存siteは変更せず拒否する。アプリ内で画像検証/transactionを再実装しない。

## 表示・品質・負例

生成directoryだけを127.0.0.1で配信。desktop相当1440×1000、mobile相当390×844を予定し、実viewport/DPR/Chrome versionを記録する。A/Bそれぞれ新規URLで、decode/参照/currentSrc/実寸法/no-upscale/WebP選択を観察し、JPEG fallbackを別の診断表示でdecodeする。placeholder含む実取得body bytesと、全variantの生成容量を分ける。

原画像・A・Bを同じ表示寸法で並べ、写真の細部/輪郭/色、合成図の文字/細線、Orientationを目視し、判断画面/cropを保存。metadataの実EXIF/GPS/XMPとICCは既存検査方式とmanifest公開契約に照らす。Agentの判断を利用者本人の承認とは呼ばない。

負例は画像でないinput、小画像WebP64/maxBytes1/maxQuality30のstrict不可能budget、成功siteへの再実行を各1回。実error code/phaseとprocess exit、成功ページ作成なし、部分生成物/対象parent内staging、既存siteのbefore/after hashを確認する。期待拒否は正常受入から除外。

## 納品

consumer/lock、raw/exit/manifest/画面/主要出力/hash、RESULTS/REPRODUCE/DECISIONを同Issueの1PRにまとめる。private pathがあれば原本はGit外、公開derivativeと原本hashの対応を保存。対象検証→セルフレビュー→Companion Gate→最終PR CI→保護merge、記録merge後確認を行う。#703はOPEN。採用目標未達でも必要な評価が揃えば不採用として完了し、別Issue/新versionへ進まない。
