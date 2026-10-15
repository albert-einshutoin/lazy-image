# #908 準備・再開手順

現時点は準備段階で、保存成功matrixのreplayではない。[PLAN](PLAN.md)と[PREPARATION](PREPARATION.md)を先に読む。`REPOSITORY`はcheckout、`HISTORY`は本directory、`NEW_PHASE`はcheckout外の新しい保存先。旧証拠と既存previewへ上書きしない。

```sh
node "$HISTORY/check-setup.cjs"
node "$HISTORY/check-observe.cjs"
node "$HISTORY/prepare.cjs" "$REPOSITORY" "$NEW_PHASE/preview"
node "$HISTORY/server.cjs" "$NEW_PHASE/preview" 8878 > "$NEW_PHASE/server.jsonl"
```

新しいpreview以外を公開しない。serverは127.0.0.1限定で、catalogにないpathは404、非GET/HEADは405、prepared fileのhash変化は500。source hashと既存出力の拒否を無効化しない。serverのfinishはOSへの引渡しで、client受信body hashではない。別fetchで元imgのbodyやcache状態を補完しない。

`catalog.json`は毎回新しいnamespaceと、case/page/asset URL・artifact・HTML/script hashを持つ。まずpilot4ページだけを使う。実Chromeの操作が利用可能になったら、viewport/DPR/zoom100%を設定し、可視DOMの `#responsive-observation` と `#decode-observation` を読む。画像要求前のobserver登録はHTML側で行う。toolのread-only evaluateでdecode/fetchを実行しない。

表示条件を変える前に `/idle.html` へ移動してからviewport/DPRを設定する。DPRは実DOMで確認し、通常zoomとDevice Toolbarの表示縮尺を混同しない。撮影が必要なら前後の実viewport/DPRを保存し、縮小された画面を画像の等倍品質や転送量の証拠へ使わない。利用者の全browser cacheや設定は消去しない。

pilotが観測契約を満たしてから、catalogのformal cold24ページを条件/試行順に開く。Mobile DPR2のwarm3組は、各primeの実decode/HTTP/cacheable応答を保存してから、同じ候補URLのresponsiveへ進む。全variant先読み・preload・診断fetchを行わない。未使用asset URL、先行requestの不在、Service Worker、選択元request、Resource Timingとserver記録を結び付ける。0や欠落だけでcacheや転送ゼロを断定しない。

正式実行前にcodeをcommitし、その実revisionとhelper/HTML/catalogのhashを保存する。rawはcase/trialと取得開始・終了を結び付け、エラー/pending/unknownも残す。観測helperやHTMLを修正したら旧実行に新revisionを付けず、影響する条件だけ別の記録として再実行する。browser操作ツール自体が利用できない場合はその障害を記録し、未観測を成功へ変換しない。

この準備チェックは新しいChromeでの選択、転送量、cache保存、codec decodeの証明ではない。画像再生成・npm install/native build・全benchmark・release matrixは不要。runtime結果が揃った後で条件別比較と運用説明を確定し、同Issueの1PRだけを納品する。
