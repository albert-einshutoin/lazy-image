# #908 技術図srcset・初回転送量の固定計画

2026-10-16。base `795de3966ce120830a23309fa37a15910aa9ebc9`。#904のSVG標準採用を維持し、保存済みの公開npm1.5.0由来WebPを再利用する。compiler、renderer、画像生成は実行しない。#902/#904/#906を再開せず、#703はOPENを維持する。

## 入力と保護

`source-manifest.json`に旧historyと原本SVGのbytes/SHA-256を固定する。入力manifestは `adoption-diagram-1.5.0-20261014/evidence/runs/B1/manifest.json`、公開製品sourceは `023a8c9368eef58f17ecc6f412363d24e5aa9b67`。保存consumer identityは過去の生成に対する証拠で、今回SDKを実ロードした結果ではない。WebPの684/960/1920pxは64,168/95,622/185,508 B。全3候補の保存容量は345,298 Bで、1表示の転送量とは分ける。

旧collector・旧HTML・旧raw・画像は変更しない。新しいtask-local previewに画像3件と必要scriptだけをコピーし、trialのURLは同じ保存bytesへのaliasとする。生成先は既存directoryを拒否する。原本browser記録と公開copyのhash対応を保存し、不要な絶対path・私有UIを公開しない。

## 固定matrixと表示

Desktop1440×1000/CSS960、Mobile390×844/CSS342（左右24px）、各DPR1/2、zoom100%。幅だけの変更をDPR2と扱わず、実DOMのviewport/DPR/表示幅を確認する。稼働Chromeのfull version、OS、操作順、撮影する場合は前後の寸法と方法を保存する。可読性・codecの再評価へ広げない。

初回は4表示条件×responsive/固定1920×各3trial＝24 page load。responsiveはmanifestのWebP URLと684/960/1920w、sizesは `(max-width: 1008px) calc(100vw - 48px), 960px`。CSSはmainの最大1008pxに左右24px、img幅100%・最大960px。固定対照も同じCSSで1920px WebPを使う。

目安はMobile684、Desktop DPR1は960、DPR2は1920だが、これを強制するscriptやsrcset除去は行わない。異なる選択も結果として保存する。formal matrixとpilotのURL集合は分ける。

## 初回と取得済みの定義

新しいrandom namespaceの各cold caseに、過去にアクセスしていない**asset URL**を3幅とも割り当てる。HTML URLだけのquery変更ではない。server全履歴でそのURLへの事前requestがないことを確認し、リンク先画像を先読みしない。global browser cacheや設定を消去しない。

画像応答は `Cache-Control: public, max-age=3600, immutable`、SHA-256のETag、Content-LengthとContent-Typeを付ける。HTML/scriptはno-store。所有preview以外は配信しないloopbackサーバーを使う。Service Worker controller/registrationの観測が不明ならunknownとし、介入を黙認しない。

補助対照はMobile DPR2の3trial。各trialの未使用namespaceで1920px固定ページを先に表示し、decode/HTTP完了とcacheable応答を保存してから、**同じ3候補URL**のresponsiveページを表示する（prime3＋対象3、計6 load）。再利用の判断にはprime、後続の選択、Resource Timing、server記録、Service Worker状態を合わせる。transferSize=0単独でcache保存や転送ゼロを断定しない。cache再利用の根拠が取れなければ未確認とする。#904当時の状態の完全再現とは扱わない。

## 観測とpilot

#906の診断scriptをbyte不変で再利用し、画像要求より前にload/error observerを登録する。probe指定を与えず、診断fetch/Blobや全variantの先読みを本測定へ入れない。新しい小さな補助scriptで観測開始/終了、実viewport/UA、Service Worker、詳細Resource Timingを可視DOMへ保存する。個別success/error/timeout/unknown、src/srcset/sizes/currentSrc、natural寸法と実rectを保持する。timeoutやsrc変更を成功へ変換しない。

最初にDesktop DPR1固定、Mobile DPR2 responsive初回、Mobile DPR2 prime→取得済みresponsiveの計4 loadをpilotとして確認する。pilotが観測契約を満たす前にformalを実行しない。表示条件を切り替える前に画像のないidle.htmlへ移動し、直前のcaseがviewport変更によって追加variantを取得することを避ける。idleは画像のpage load数に加算しない。欠落・失敗は保存し、修正したcode/HTMLに旧revisionを付けない。正式測定の概算10〜15分（正常にbrowser操作できる場合）。

```sh
node "$HISTORY/check-setup.cjs"
node "$HISTORY/prepare.cjs" "$REPOSITORY" "$NEW_PREVIEW"
node "$HISTORY/server.cjs" "$NEW_PREVIEW" 8878 > "$NEW_PHASE/server.jsonl"
```

生成されたcatalogのpage URLだけを所有tabで開き、read-only tool evaluateでは可視DOMの記録を読む。decode/fetchはtool evaluate内で実行しない。browser機能やsecurity policyが利用不能なら回避せず、必須観測を未実施として残す。

## 集計と判断の境界

trial/page/asset URL、HTML/collector/code hashと実行revision、環境、server request ID/headers/status/完了または中断、全追加variant/重複requestを結び付ける。serverの完了は応答をOSへ渡した範囲で、client受信bodyの直接hashではない。選択ファイルのencoded幅・bytes/hashは固定manifestと配信bodyから照合する。naturalWidthは密度補正を受け得るためencoded幅として使わない。

画像body bytes、Resource Timing上のtransferSize/encodedBodySize/decodedBodySize、全候補保存容量を別に示す。rawのresourcesは全リソース、imageRequestsは候補画像URLだけで、重複を除去しない。scriptを画像転送量へ加算しない。転送削減率は同じ表示条件のresponsiveと固定を比較し、追加request分も含める。Resource Timingはbrowser APIの観測値で、実packetや正確なHTTP header byte数ではない（[W3C Resource Timing](https://www.w3.org/TR/resource-timing/#dom-performanceresourcetiming-transfersize)）。未取得・0・情報欠落を別fetchや期待値で埋めない。速度/LCP/CDN/SLAや全browserへ一般化しない。

候補選択は[HTML Standard](https://html.spec.whatwg.org/multipage/images.html#selecting-an-image-source)の仕組みと実設定に照らして判断する。小さい候補を選ばない場合や削減がない場合も有効な評価だが、必須観測を実施できない状態は未完了とする。

## 納品

raw/失敗/unknown、条件別RESULTS、DECISION、REPRODUCE、対象検証、導入ガイドの運用説明を同Issueの1PRへまとめる。対象検証→セルフレビュー→必要な指摘修正/最終確認→今回差分のCompanion Gate後に最終hosted検証。HTTP観測と証拠の境界は専門レビュー1件で確認する。最終HEADの必須チェック成功後に保護mergeし、同merge SHAのmain CI/Securityと成果物を照合する。必須確認が欠ける間はIssue/Goalを閉じない。製品/API/codec/依存/CI/release/deploy、別Issue、branch削除へ進まない。
