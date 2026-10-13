# #904 再実行手順

対象は公開npm 1.5.0 / source `023a8c9368eef58f17ecc6f412363d24e5aa9b67`、新規SVG1点。旧#902/20261013準備へ上書きしない。[PLAN](PLAN.md)の文字/接続/表示/予算を先に固定する。結果・制約は[RESULTS](RESULTS.md)、採用条件は[DECISION](DECISION.md)。

## 消費側と入力の同一性

`REPOSITORY`はこのcheckout、`HISTORY`は本directory、`NEW_PHASE`と`NEW_CONSUMER`はcheckout外の新規directoryを指定する。既存出力先を削除して再利用しない。保存native hashはdarwin-arm64に限定し、他platformのbinaryを暗黙に同一扱いしない。

```sh
cp -R "$HISTORY/consumer" "$NEW_CONSUMER"
cd "$NEW_CONSUMER"
npm ci --registry=https://registry.npmjs.org/ --no-audit --no-fund
node --input-type=module -e 'import {loadPublished} from "./published.mjs"; console.log(loadPublished().identity)'
```

通常のsave-exact installは20261013準備で済み、今回lock/実JS5件/実ロードnative/version/overrideなしを再照合した。`published.mjs`はこのconsumer内のinstallを要求し、root SDK、native override、NODE_PATH/NODE_OPTIONS、異なるlock/JS/native hashを拒否する。公開native1.5.0のhashは `d7b3871a3377f0add70a8058992505207e047728db0dcef8ff44654838b98055`。新しい環境で違うidentityになったら旧証拠へ上書きせず、環境差として扱う。

## 測定外のSVGレンダリングとlossless PNG

既存checkoutのsharp0.35.4/librsvg2.62.91をrendererとして使う。製品native buildは不要。Arial regular/boldと保存hashを利用環境で照合し、fontをPRへコピーしない。

```sh
mkdir "$NEW_PHASE"
node "$HISTORY/render.cjs" "$REPOSITORY" \
  "$REPOSITORY/docs/images/compiler-artifact-flow.svg" "$NEW_PHASE/raster"
cd "$NEW_CONSUMER"
node controls.mjs "$NEW_PHASE/raster/diagram-input.png" "$NEW_PHASE/controls"
```

`render.json`のSVG/input hash、RGB/no alpha、1920×2800、白背景/density144/version/fontを確認する。`input.json`と一致しないPNGをcompileしたことにしない。SVG→PNGは外部処理、PNG対照は公開低レベルImageEngineのsanitize/resize/toFile('png')による測定外診断。compilerのSVG/PDF入力・PNG出力や失敗fallbackではない。

## A1とB3trial

測定時helperは `a8aa32716c8ce9fa4078808a9424e12969171a7f`の4ファイル。実行するcodeを保存したrevisionと実file hashへ結び付ける。今後変更したcodeに旧revisionを付けない。

```sh
cd "$NEW_CONSUMER"
EVALUATION_REVISION=<actual-helper-revision> \
  node measure.mjs "$NEW_PHASE/raster/diagram-input.png" "$NEW_PHASE/runs"
node "$HISTORY/check-outputs.cjs" "$REPOSITORY" "$NEW_PHASE"
```

順序A1/B1/B2/B3、各fresh process、新output directory、画像1点/6 artifact。wrapperは実command/exit/signal/spawnError/process wall/ファイルhashを保存し、失敗も保持する。imageStageはcompileImage直前〜resolve/rejectのみ。renderer/PNG対照/HTML/監査/目視を混ぜない。B imageStage≤60000msを確認。既存resource制限を延長せず、E123/E300の失敗を低レベルAPIの生成で置き換えない。

保存済み成果物の再照合は画像を再生成しない。

```sh
node "$HISTORY/check-outputs.cjs" "$REPOSITORY" "$HISTORY/evidence" --check
node "$HISTORY/check-browser.cjs" "$REPOSITORY" "$HISTORY" --check
```

これらは今回の保存成功matrix/観測を対象とするチェック。異なる実行が失敗したらrawを保持し、保存成功の再現とは扱わない。

## browser表示と原図拡大

```sh
mkdir "$NEW_PHASE/preview"
cp "$HISTORY/preview/qa.js" "$NEW_PHASE/preview/qa.js"
node "$HISTORY/preview.cjs" "$REPOSITORY" "$NEW_PHASE" "$NEW_PHASE/preview/final"
python3 -m http.server 8876 --bind 127.0.0.1 --directory "$NEW_PHASE/preview"
```

保存した[preview](preview/)は静的配信で読める。SVG、各幅のpng/webp/jpeg単体ページ、picture.html、detail-svg.htmlを小さな個別表示で確認する。qa.jsは既存#902のbrowser内Image.decode/Resource Timing→可視DOM観測方式を再利用した。tool側の未提供decode methodやcompleteだけのチェックと混同しない。

Desktop1440×1000/CSS960、Mobile390×844/CSS342、各DPR1/2。今回の稼働Chromeは154.0.8037.98、Desktop種別とMobile(no touch)種別を使用。browser zoom100%、Device Toolbar明示75%。DevToolsをundockし、幅/高さ/DPR設定後に**実DOMの寸法/DPR**を確認する。UIの表示値だけでは不十分だった。viewport0件/不一致を成功にしない。

通常tab screenshotは校正時にviewport/DPRを変えたため使用しない。native App.getScreenshotのJPEGを使い、撮影前後の実viewport/DPRが同じことを保存。100 CSS px ×24pxの黒いrulerで撮影scaleを測る。[crop-native.cjs](crop-native.cjs)は75 pixel/100 CSS pxを要求し、図だけをPNGへ切り出す。PNG化は元の撮影JPEG損失を回復しない。scaled画面を等倍source-pixel/物理DPR2の画質として扱わない。

```sh
node "$HISTORY/crop-native.cjs" "$REPOSITORY" \
  "$PRIVATE_NATIVE_CAPTURE" "$CAPTURE_JSON" "$NEW_PUBLIC_CROP"
node "$HISTORY/crop-files.cjs" "$REPOSITORY" "$NEW_PHASE" "$NEW_PHASE/file-crops"
```

実decode/UA/full version、naturalWidth、encoded寸法/file hash、currentSrc、sizes、CSS rect、scroll/clip、native撮影前後のviewport/DPRを記録する。[実行したcollector snapshot](evidence/collector-source-v2.txt)と[拡大snapshot](evidence/collector-detail-source.txt)はcua_replの初期化済みbinding/出力先/4caseを前提とし、一般的browser CLIではない。Native原本は私有window UIを含み得るためローカルのみ、公開用には図cropと対応hashを使う。

拡大viewerは原図960pxをscroll領域に置き、Original size/2×/Verification/Failure/Left/Rightを実キーボードで操作した。Mobileはverification/failureの横位置0/272/544/620、Desktopは各段階、全4条件で2×1920pxも保存。Native widgetやgeneric locatorのmouse/visible判定が失敗した操作を成功に読み替えず、Tab/Spaceによる実フォーカス・scroll値・画像rectで確認する。

原図を同CSS寸法、同resizeのPNG、最終B WebP/JPEGで比較。pixel cropは実decodeしたfile pixelsをresizeせず抽出し、lossless PNG保存する。browser撮影のJPEGノイズをcodecの差へ帰属しない。Source SVG↔PNGの差にはrenderer/font/resize/displayが含まれ、縮小だけの因果を断定しない。

## 保存範囲・失敗

原本は今回のtask-local 20261014 phase、準備原本は別20261013 directoryに残す。公開コピーは[LOG_REDACTIONS](LOG_REDACTIONS.json)で原本/derivative hashを対応付ける。full-window原本、node_modules、private path、巨大な一時contact sheetはPRへ含めない。

EEXIST copy保護、入力hash拒否、撮影method/visible/touch入力の失敗、初期detailの自動縮小を正常compiler受入へ加算しない。旧#902のdecode2件は今回未解明を保持。Source/viewport/予算/採否/最終CIはそれぞれ今回のrevisionへ結び付け、旧release/CI成功を流用しない。全build/benchmark/release12環境の再実行をこの評価の再現手順へ追加しない。
