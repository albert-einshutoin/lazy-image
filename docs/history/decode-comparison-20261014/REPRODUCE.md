# #906 再実行

既存#902 historyは読み取りのみ。`REPOSITORY`はcheckout、`HISTORY`はこのdirectory、`NEW_PHASE`はcheckout外の新規directory。今回の保存rawへ上書きしない。

```sh
node "$HISTORY/check-observer.test.cjs"
node "$HISTORY/check-prepare.test.cjs"
node "$HISTORY/check-records.cjs" "$REPOSITORY" "$HISTORY"
node "$HISTORY/prepare.cjs" "$REPOSITORY" "$NEW_PHASE/preview"
python3 -m http.server 8877 --bind 127.0.0.1 --directory "$NEW_PHASE/preview"
```

既存directoryをprepareへ渡すと拒否する。依存追加/native build/npm install/compiler再実行は不要。既存Sharpで画像の実pixel decodeを確認する場合は`sharp(file).raw().toBuffer({resolveWithObject:true})`を使用し、metadataだけで成功判定しない。対象2file/hashは[RESULTS](RESULTS.md)、Sharp版本/decoded hashは[image-check](image-check.json)。

matrix.jsonの6ページを各3回、trialごとに記載順で開く。viewport1440×1000、DPR1、zoom100%を実DOMで確認。HTML queryは変えてもasset cacheはcoldにならない。[baseline collector snapshot](collector-baseline-source.txt)は初期化したCUAのowned tab/fs/hash bindingを前提とする実行snapshotで、汎用browser CLIではない。qa.jsのbrowser内image.decode結果を可視DOMから読み取り、loadingをJSONとして保存しない。toolのread-only evaluate内でdecode/fetchを実行しない。

diagnostic-quality.html/diagnostic-quality-fresh.htmlを各3回、control-broken.html/control-missing.html/control-invalid.htmlを各1回開き、`#decode-observation`を読む。[diagnostic collector](collector-diagnostic-source.txt)がURL、実viewport、source/HTML/observer hash、pendingを保存する。case/trialとraw filenameを結合し、新実行のcode revision/実hashへ結び直す。旧freeze commitを変更codeに付けない。

個別decodeの観測上限10秒、collector待機30秒。native decodeの強制停止ではない。late結果を保存timeoutへ上書きしない。HTTP probeは別のno-store fetchとBlob decodeなので元imgのbodyとは分ける。正常と制御された失敗を別集計にする。64 B PNGのacceptは記録されたChromeだけの結果で、全browserに同じ対照結果を要求しない。

`check-records.cjs`は今回の保存matrixとhashを検証する。別環境の失敗/未再現を今回の成功へ書き換えるreplayではない。異なる新結果は別directoryへ残し、browser/環境/取得状態を記録する。新しい撮影を行う場合は実viewport/DPRを前後で確認し、撮影による変更や縮小を画質証拠にしない。今回のsummary screenshotはmatrix終了後の結果表で、decodeや画質そのものの証拠ではない。

旧証拠保護はsource-manifestの661fileを再hashして確認する。公開textはprivate path等を検査し、必要なredactionは原本/公開hashを台帳へ残す。旧#902は変更/再開せず、当時原因未確定を維持する。最終HEADのreview/CIとmerge SHA/main確認はIssue/PRへ記録し、過去のALLOW/CIを流用しない。
