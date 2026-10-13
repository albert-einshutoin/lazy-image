# #904 採用判断

**SVGを標準採用。Desktopは最大960 CSS px、Mobileは342 CSS px＋原図拡大を採用条件とする。** 今回新規作成した導入図に対するAgent判定で、利用者本人の画質承認や実利用者の採用実績ではない。

| 項目 | 確定した運用 |
|---|---|
| 原本/標準形式 | 編集可能な白背景SVG、960×1400 viewBox、4834 B ≤100 KiB、MIT |
| Desktop | viewport1440×1000、CSS最大960px、DPR1/2。主要/詳細ラベルと順序、verified only、失敗の意味を読む |
| Mobile | viewport390×844、左右24px、CSS342px、DPR1/2。6工程を把握し、詳細は原図960pxへ移動してscroll/zoom |
| 拡大導線 | 元SVGへのリンク。評価viewerでは原寸960 CSS px、段階/左右移動と2×(1920px)を確認 |
| ラスター主形式 | WebP、684/960/1920px、maxBytes102400/204800/409600、maxQuality80 |
| JPEG | 不透明PNG入力の任意fallback。全候補は可読だがbytesと輪郭にじみが増える。maxBytes153600/307200/614400、maxQuality85 |
| 最小幅 | 検証した候補内の最小可読幅は684px。Desktop960 CSS pxでは表示上拡大されるため、960/1920も用意 |
| srcset/sizes | 実manifestから684/960/1920wを生成。sizesは`(max-width: 1008px) calc(100vw - 48px), 960px` |

[実bytes/quality/生成時間と表示結果](RESULTS.md)の全候補を比較済み。SVGはラスタより小さく、文字/矢印を編集・拡大できる。公開compilerのSVG直接対応として扱わず、その配信は通常の静的assetとしてアプリに任せる。

ラスタが必要なpipelineでは、測定した[6予算policy](consumer/policy-B.json)でverified画像/manifestを得られる。B3trialは各1.332–1.350秒、18/18 budget適合、q80/q85、A/B画像は同一bytes/hash。今回の上限で追加圧縮を得た証拠ではない。PNG対照は測定外の低レベル診断で、compiler成功の代用ではない。

通常Mobileで詳細を全部読める条件にはしない。原図を開く導線を省く場合は今回の採用条件を満たさない。font環境、入力内容、表示幅、予算が変わったら必須の文字・方向・凡例を再確認する。maxQualityは最低品質/時間保証ではなく、E123/E300を失敗のまま扱う。codec強制停止、全siteのatomic publish、外部配信やSLAを保証しない。

DPRはChromeのemulationで、native画面記録は75%表示＋撮影JPEG損失を明示した。1:1 decoded-file cropも確認したが物理Retina/実機tap/全browserの承認ではない。warm cacheのresponsive確認では4条件とも1920pxを選択し、Mobile転送量削減を実測したとはしない。旧#902のdecode2件は未解明を保持する。

今回の図・ガイド・評価を1 PRで納品し、#904の条件を満たした後に閉じる。#703はOPEN。製品実装、次Issue、release/deployへ進まない。
