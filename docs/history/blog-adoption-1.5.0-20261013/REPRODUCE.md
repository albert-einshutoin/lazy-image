# 再現手順 (#902)

対象packageは公開npm @alberteinshutoin/lazy-image **1.5.0**。公開sourceは`023a8c9368eef58f17ecc6f412363d24e5aa9b67`。評価コードfreeze `6ffda40cd1af311711d17c178e838ad842f1d64f`と測定raw内hash、追加観測helperのhashを分けて読む。実行時の補助code snapshotはevidence/executed-auxiliaryに保持し、保存用helperへの引数追加と区別する。実測環境はmacOS27.0 / M4 arm64 / Node24.2.0 / npm11.3.0。prepareのbinding確認もこの1環境に限定する。全platformで同じhash/時間を保証しない。

## 新規consumerと単一page command

checkout外の新しいdirectory（末尾consumer）へ保存consumerをコピーし、node_modules・過去の出力を持ち込まない。binding override / NODE_PATH / NODE_OPTIONSを設定せず、npm link・local tarball・checkout SDKを使わない。`npm ci`で保存lockどおり公開registryから通常installする。最初の実測時はsave-exact installした[実log/exit](evidence/install.log)とlockを保持している。

```sh
# CONSUMERはcheckout外の新規保存先。
cp -R docs/history/blog-adoption-1.5.0-20261013/consumer "$CONSUMER"
cd "$CONSUMER"
npm ci --registry=https://registry.npmjs.org/ --no-audit --no-fund

# 既存のtrusted parentに毎回新siteと新reportを選ぶ。
# この一つのcommandで6画像→全compile成功後の記事HTMLを再buildする。
npm run build -- B "$NEW_SITE_DIR" "$NEW_REPORT_FILE"
# Aの場合は B を A へ替える。
```

保存consumerには6入力・license・具体policyがある。fixtureを準備し直す場合はcode/package/lockだけを新consumerへコピーしてnpm ci後、`node prepare.mjs "$REPOSITORY"`。inputs/policiesを既に持つdirectoryではprepareを再実行しない。MIT/CC0原本・corpusを参照し、生成されたcoffee-64のhashを新たに記録する。prepareは既知fixture専用で、bounded inspectのorientationKnown=falseをtrueへcoerceしない。

## A/B各3trial

```sh
node build.mjs --self-test
EVALUATION_REVISION=6ffda40cd1af311711d17c178e838ad842f1d64f \
  node measure.mjs "$NEW_RUNS_DIR"
node audit.mjs "$NEW_RUNS_DIR"
```

EVALUATION_REVISIONは実際に使うconsumerを保存したrevisionへ更新し、code hashも照合する。旧revisionで変更コードを測ったことにしない。measureはA1,B1,A2,B2,A3,B3、fresh process、6画像逐次。成功を保証せず、exit/signal/error/rawと残るfilesを保存する。auditはこの保存成功matrixのsource/manifest/budget/実filesを確認するチェックで、別環境の失敗を成功へ変換しない。新結果は今回の保存先へ上書きしない。

```sh
# 保存した成功siteを明示し、各負例を新出力先へ1回だけ。
node negative.mjs "$NEW_NEGATIVE_DIR" "$NEW_RUNS_DIR/B1"
```

既存siteは削除せず、失敗rawを保存する。negativeの第2引数を省略すると今回の専用配置 `consumer/../runs/B1` を使う。

## 表示と品質

測定外helperを使って今回と同じ専用配置の新previewを作る。qa.jsはbrowser内decodeとResource Timingを可視DOMへ出す観測コードで、通常consumerのpageには含まれない。preview.mjs/quality-pages.mjs/qa.jsのhashを保存する。

```sh
node preview.mjs "$NEW_PREVIEW_DIR" "$NEW_RUNS_DIR"
node quality-pages.mjs "$NEW_PREVIEW_DIR" "$NEW_RUNS_DIR"
python3 -m http.server 8875 --bind 127.0.0.1 --directory "$NEW_PREVIEW_DIR"
```

保存した[preview](preview/)は実際に確認したHTML/qa.js/参照画像/成果物を含み、静的配信だけでも確認できる。再現helperの生成確認では、主buildを再実行せず、新previewのHTML/画像hashが観測済みpreviewと一致することを対象確認した。観測用4URLはdesktop/mobile A/Bの*-observed/index.html、JPEGはdesktop-B-observed/qa-jpeg.html、品質はquality-single/<input>/index.html。1440×1000と390×844で実viewport/DPR/稼働中browserのfull versionを記録し、qa-observationがloadingからJSONへ更新するまで待つ。browser内の実decodeをDOMへ表示した結果を保存し、CUA読取DOMの未提供decode methodと混同しない。

画面は原図とA/Bの同じ表示寸法を確認する。大入力は960×960表示の中央crop、metadataの参照は公開ImageEngineでOrientation正規化したPNG（測定外）。Resource Timingのbodyと実fileを対応付け、全生成variant容量/HTML/manifestと、実取得画像容量を分ける。Cache/URLを変えた観測は別記録として保存する。

## 保存範囲

[evidence](evidence/)にidentity、実command/exit、全6buildと負例、metadata、browser、初期比較失敗を保存。[LOG_REDACTIONS.json](LOG_REDACTIONS.json)で原本と公開derivativeのhash/bytesを結び、private pathだけを置換する。原本はGit外の専用評価directoryへ保存した。[evidence-hashes.json](evidence-hashes.json)は公開filesとcodeを固定する。

本体build、全benchmark、公開12platform smoke、release/tag/publish/deployをこの評価の再現commandへ追加していない。採用可否は[DECISION](DECISION.md)、未達・観測できない値は[RESULTS](RESULTS.md)を正本とする。
