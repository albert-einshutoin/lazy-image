# #904: 導入ガイドの新規技術図と公開1.5.0評価

2026-10-14。利用者が今回新規作成を許可した掲載候補1点。旧テスト画像や提供されていない実利用図ではない。SVGを標準掲載形式とする方針は測定前の指定で、ラスター比較の合否とは別。

原本: [compiler-artifact-flow.svg](../../images/compiler-artifact-flow.svg)。白背景、960×1400 viewBox、編集可能なtext/path、Arial。公開source `023a8c9368eef58f17ecc6f412363d24e5aa9b67`のcompiler/APIと現導入ガイド・static-site例に基づき新規作成。MIT、root LICENSE適用。私有画像・第三者の図を使わない。

## 測定前の内容・可読性基準

主要ラベルは「1 Input image + policy」「2 compileImage()」「3 Private staging」「4 Local commit」「5 Images + manifest」「6 HTML + delivery」。この順に下向き矢印で結ぶ。3→4はverified only。赤い失敗分岐はエラーと未確定stagingのcleanupへ向かい、アプリ配信へ接続しない。

Desktop通常表示では6ラベル、矢印の順序、private stagingで生成・hash/metadata検証、budget適合、verified only、1 directory rename、既存output拒否、artifact setのみの確定、アプリのHTML/配信責任、失敗時に未検証成果物を確定しない意味を読めること。

Mobile通常表示では6つの主要段階と矢印の順序・compilerとapplicationの区切りを把握できること。詳細は原図の960 CSS px表示へ移動して拡大確認できることを採用条件にする。拡大表示では上記の全詳細、cleanupError、srcset/sizes、storage/CDN/remote publish/site rollbackがアプリ責任であることを読めること。

色だけを意味の唯一の手掛かりにせず、ラベルと破線も使う。内容とSVG通常表示を確認後、測定対象のSVG hashを固定する。以後変更した場合は依存するPNG・compiler測定・表示確認を更新する。結果を見て基準を除外・緩和しない。Agentの判定を利用者本人の画質承認と呼ばない。

## 今回決めた表示条件

| ケース | viewport | 図の通常CSS幅 | DPR |
|---|---|---|---|
| desktop-1 | 1440×1000 | 960px | 1 |
| desktop-2 | 1440×1000 | 960px | 2 |
| mobile-1 | 390×844 | 342px（左右24px） | 1 |
| mobile-2 | 390×844 | 342px（左右24px） | 2 |

既存本番サイトや実機の観測条件ではなく、今回指定された掲載/評価条件。browser zoomとDevice Toolbar表示zoomを固定・記録する。Fit to windowの縮小shotを等倍画質の根拠にしない。各画像の実decode、naturalWidth、CSS rect、currentSrc、DPR、browser version、撮影倍率と実shot寸法を保存する。小さな個別表示で比較し、前回18画像sheetの未解明decode2件を解消済みと扱わない。

## 入力・対照・固定policy

SVGを既存sharp/librsvgで1920×2800の不透明RGB PNGへ外部レンダリングする。renderer/version、選択font、背景、SVG/PNG hashを記録。公開ImageEngineの同じresize({width,fit:'inside'})を用いた684/960/1920pxのlossless PNG対照は測定外の診断用。SVG/PDF入力・PNG出力をcompileImageの機能として説明しない。

公開1.5.0の独立consumerはlock・実ロードJS/native・overrideなしを現在照合して再利用する。root native buildで置き換えない。

| 幅 | WebP maxBytes | JPEG maxBytes | WebP maxQuality | JPEG maxQuality |
|---|---:|---:|---:|---:|
| 684 | 102400 | 153600 | 80 | 85 |
| 960 | 204800 | 307200 | 80 | 85 |
| 1920 | 409600 | 614400 | 80 | 85 |

SVG上限は102400 B。すべて設計上限であり、実測値ではない。1 KiB=1024 B。両policyはwidths=[684,960,1920]、formats=['webp','jpeg']、placeholder=false。Aはbudgetsなしの既定quality WebP80/JPEG85。Bは上表の6budgetsを明示。compilerにfixed quality/minQuality/timeout overrideを追加しない。maxQualityは最低品質・受入・時間保証ではない。

Aを新規出力先へ1回、最終ラスター比較policy Bを新規出力先へ3回。6 artifact/呼出し、合計4呼出しの予定。encode内部の実行回数は推測しない。imageStageMsはcompileImage呼出し直前〜resolve/rejectまで、外側process wallも別記録。図作成・SVGレンダリング・PNG対照・監査・HTML作成・目視はimageStageMsへ含めない。各B imageStageMsが60000ms以内か確認する。既存の5秒resource検査を延長せず、E123/E300等・exit/signal・未公開状態を保存する。

## 表示比較・納品

SVGと各幅のPNG/WebP/JPEGを同CSS表示で比較し、縮小/表示条件と非可逆圧縮の影響を区別できる範囲だけ述べる。A/B bytes/hashが異なれば最終B成果物を再確認する。srcsetを使う配信例は実manifestから作り、sizesにはCSS幅を記述。naturalWidthとencoded寸法、body bytes/transfer/全variant容量を区別する。

図・ガイド・consumer・同一性・raw/exit/manifest/画面・crop・RESULTS/DECISION/REPRODUCEを対応PR1件へ保存する。原本と公開用資料を別hashで対応付け、不要な絶対パスとnode_modules・大きなrenderer一時ログをPRへ含めない。旧#902証拠と20261013準備は不変。

対象検証→セルフレビュー→Companion Stop Review Gate→最終必要hosted CI→merge→同merge SHAのmain確認→#904完了。#703はOPEN。製品・API・codec・依存・CI基盤・release/deploy変更、旧release matrix/全6画像再測定、別Issueへの着手は範囲外。
