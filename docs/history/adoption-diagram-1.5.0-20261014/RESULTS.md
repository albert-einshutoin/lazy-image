# #904 評価結果

2026-10-14。導入ガイド用に今回新規作成した1点の評価。既存利用者の実図・採用実績ではない。公開npm 1.5.0、source `023a8c9368eef58f17ecc6f412363d24e5aa9b67`を独立consumerから使用。測定helper保存revisionは `a8aa32716c8ce9fa4078808a9424e12969171a7f`。公開製品source、評価codeのfreeze、後の納品HEADは別。

## 入力・実行範囲

SVG原本4,834 B、SHA-256 `71fb80cc85bcf53a9042234973d476638ae9338fc892bca8795714d66b89e7d8`。白背景960×1400、MIT。既存sharp0.35.4 / librsvg2.62.91 / libvips8.18.6、density144で不透明RGB/sRGB PNG1920×2800を作成。入力427,223 B、SHA-256 `fd4966526149ea92d6dc0ec88bd01581c4ad2e0604c7fbac3e0f4ff2b9899504`。font-familyはArial、利用環境のArial regular/boldファイルhashを[render記録](evidence/raster/render.json)へ保存。fontファイルは配布しない。別OSでの同一font描画は保証しない。

環境はApple M4、10 CPU、32GiB、macOS27.0 (26A428)、Node24.2.0、npm11.3.0。[環境](evidence/environment.json)と[consumer identity](consumer/identity.json)を参照。20261013の通常installを今回再利用し、lock、JS5件、実ロードnativeのhash、overrideなしを現在照合した。nativeは1.5.0 darwin-arm64、SHA-256 `d7b3871a3377f0add70a8058992505207e047728db0dcef8ff44654838b98055`。

imageStageMsはcompileImageの呼出し直前からresolve/rejectまでで、生成・検証・ローカル確定を含む。Node起動/import/report保存を含む外側process wallは別。図の作成、外部レンダリング、低レベルPNG対照、HTML生成、画面撮影、監査・目視は含めない。manifest未提供のnative metricsとencode内部実行回数は推測しない。

## 生成・予算

| 呼出し | 実exit | imageStage ms | process wall ms | 成果物 |
|---|---:|---:|---:|---|
| A1 予算なし | 0 | 1358.011 | 1445.581 | 6/6 |
| B1 固定予算 | 0 | 1340.117 | 1392.331 | 6/6 |
| B2 固定予算 | 0 | 1350.398 | 1400.180 | 6/6 |
| B3 固定予算 | 0 | 1331.978 | 1386.505 | 6/6 |

BのimageStage中央値1.340秒、最大1.350秒、3/3で60秒以内。正常呼出し4/4、正常成果物24/24、Bのbudget18/18適合。E123/E300/その他compiler失敗は今回0件。期待拒否を正常受入へ加算していない。全呼出しの画像bytes/hashは同一。manifestには予算情報の差がある。余裕のある今回のbudgetで追加圧縮や高速化を得たとはしない。SLA、p99、全入力/全platformの受入率は未測定。

| 幅×高さ | PNG対照 B | WebP B / q / 上限B | JPEG B / q / 上限B |
|---|---:|---|---|
| 684×998 | 217959 | 64168 / 80 / 102400 | 105685 / 85 / 153600 |
| 960×1400 | 325656 | 95622 / 80 / 204800 | 161979 / 85 / 307200 |
| 1920×2800 | 326875 | 185508 / 80 / 409600 | 406976 / 85 / 614400 |

上限は測定前に指定された設計値。PNGは同じ公開ImageEngineのsanitize＋resize({width,fit:'inside'})によるlossless診断用で、compilerのPNG成果物ではない。[raw/exit/実manifest/ファイル](evidence/runs/)、[全出力照合](evidence/output-check.json)、[PNG対照](evidence/controls/)を保存した。SVG→PNGもcompilerのSVG対応や性能測定へ含めない。

## 4表示条件と可読性

稼働中Chrome154.0.8037.98。Desktop1440×1000 / CSS960、Mobile390×844 / CSS342、各DPR1/2を実値で確認。DesktopはDesktop emulation、MobileはMobile(no touch) emulation。実機/tap操作の証拠ではない。

SVG＋3幅×PNG/WebP/JPEGの40 image loadとresponsive例4 loadがdecodeし、拡大SVGの4 page loadもdecodeした。通常66 frame、拡大24 frame、合計90記録で撮影前後のviewport/DPRを照合。[browser照合](browser-check.json)、[各観測JSONと画面crop](evidence/browser/)を保存。viewportを狭めただけのDPR2ではない。

`qa`はページ読込・decode時の観測、`before/after`は各撮影時の実値である。拡大ページは同一画像を保持して表示条件を切り替えたため、読込時の`qa.viewport`/UAが後の撮影条件と異なる記録がある。撮影条件の判定は`before/after.viewport`、画像rect、panを使い、古い読込値を現在値として扱わない。

browser zoomは100%。Device Toolbar表示は明示75%で、100 CSS pxの黒いrulerが75 pixelになることを各native画面で測定した。画面取得はcua App.getScreenshotのJPEG、公開PNG cropへ変換しても元の撮影圧縮は残る。これらは75%の画面記録で、等倍source-pixel/物理Retina画質の証拠ではない。補助として[実ファイルの1:1 decoded-pixel crop](evidence/file-crops/)をlossless PNGで保存し、撮影のJPEG損失とcodec差を混同しない。

Agentは各表示記録とpixel cropで次を確認した。利用者本人の画質承認とは別。

| 候補 | Desktop通常の主要/詳細ラベル・接続 | Mobile通常の6工程 | 詳細条件/所見 |
|---|---|---|---|
| SVG原本 | PASS | PASS | 原図960 CSS pxへ移動し横/縦スクロール、2×選択も確認 |
| PNG684 / 960 / 1920 対照 | 各PASS | 各PASS | 縮小後の字形/境界の平滑化はあるが意味を識別 |
| WebP684 / 960 / 1920 | 各PASS | 各PASS | 対照より軽い輪郭の平滑化。必須文字、verified only、赤い失敗分岐を識別 |
| JPEG684 / 960 / 1920 | 各PASS | 各PASS | 対照より輪郭周辺のにじみ/ノイズ、赤線の柔らかさ。必須文字は識別 |

Mobileの詳細文字を通常342pxで全部読める判定にはしない。原図960 CSS pxのscroll領域で、verificationの水平位置0/272/544/620とfailureの同位置を保存し、全文を確認した。2×表示は1920 CSS px、viewportは保つ。Desktop通常は上/下を保存し、local compiler枠、アプリ責任、未検証成果物の非確定、cleanupErrorを確認した。

SVGとPNG対照の差には外部rasterization、font描画、resize、browser表示が含まれるのでresizeだけを原因と断定しない。同じnative resize後のPNGとWebP/JPEGのpixel crop差は非可逆encode/decodeを含む。文字と意味を保持する結果を、この図以外の細線や全画像に一般化しない。

## srcsetと取得の範囲

responsive例のsizesはCSS幅で`(max-width: 1008px) calc(100vw - 48px), 960px`、実manifestの684/960/1920wを使う。全4条件でcurrentSrcは1920px WebP (185508 B)だった。比較で各variantを先に読んだwarm cacheの状況であり、Mobileが684pxを取得して節約したという結果ではない。

naturalWidthはDesktop960、Mobile342でdensity補正済み。encoded幅1920と混同しない。image body bytes、Resource TimingのtransferSize/cache値、全variantの保存容量も別。JPEG個別表示は診断で、非WebP browserのfallback negotiationは今回未実施。

## 失敗・制約

準備コピーのEEXISTは既存lock保護でexit1、hash照合後に未コピーhelperだけ追加。入力hash不一致の対象チェックはcompiler前にexit1、imageStage未開始/出力なしを確認。これらはcompilerの正常受入/codec失敗ではない。

撮影校正では、ブラウザtab screenshotがviewport/DPRを変えたため除外。native full-size PNG downloadはUI完了表示のみで保存先未確認、Downloads内部URLへのアクセスはbrowser security policyで拒否され、別surfaceで回避しなかった。採用したnative画面撮影は撮影前後の寸法/DPRを保持する。緑4px rulerは撮影JPEG損失で検出できず、黒24px rulerへ変更して校正。locatorのvisible待機とtouch-mode mouse操作の失敗も保存区分を分ける。

初期拡大ページはMobile layout viewportが1560×3376へ広がって自動縮小したため不合格として[HTML/JSON](evidence/calibration/)を保持。scroll領域へ変更し、960px原図/2×/段階移動/左右操作を実キーボードで確認した。通常11 HTMLは修正前後でbyte同一、SVG/input/codec出力は不変なので画像処理は再実行していない。

#902の18画像sheet decode失敗2件は今回再調査しておらず、未解明のまま。今回の48 page load成功を過去の解消として扱わない。原本画面はGit外保管、公開資料は図だけのcropとpath置換derivative。[対応hash](LOG_REDACTIONS.json)を保持する。

専門レビュー後、測定の入力・policy・公開package・helper・実command・exit・ファイル一覧と、撮影の完全pose集合・pan・原本JPEG/capture JSON/cropのhashを対象checkerへ追加した。90 descriptorのcapture JSON hashは実原本と既存cropを再照合してから追加し、画像・撮影記録・時間を作り直していない。旧descriptorはローカルに保持した。検証結果は[VALIDATION](VALIDATION.md)を参照。
