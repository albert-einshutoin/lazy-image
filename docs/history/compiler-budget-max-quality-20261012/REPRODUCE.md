# #889の再実行

保存証拠を上書きしない。必要なら新しいowned DATA/TEMP/checkout pathsへrunnerを明示変更し、別source/runner hashで新しい評価として扱う。

1. source `574b81148b4f2fc30ddab567644e1037c2416343` のclean checkoutと既存fixturesを用意する。
2. lock対応npm ci、独立Cargo targetでcanonical npm run build。SDK/CC/CLI/command/exitはevidence/setup.jsonを参照する。global設定・依存・Rustを変更しない。
3. canonical生成型を持つsourceとbuild inputsを照合し、実native/generated hashesと実command/exitの新setup.jsonを保存する。旧receiptを新build成功として使わない。
4. 新owned DATAのsetupをrunnerに結び、fixture/license/source/JS/nativeを確認して凍結する。

```sh
node docs/history/compiler-budget-max-quality-20261012/evaluate.cjs --freeze
node docs/history/compiler-budget-max-quality-20261012/evaluate.cjs --run
node docs/history/compiler-budget-max-quality-20261012/evaluate.cjs --proof
```

24主操作・4限定回帰・35独立quality対照。worker guard600秒、CLI guard120秒は測定の保護で、製品5秒制限を変更しない。
worker failure/signalはmeasurement failureとして保存し、成功へ修復しない。source・JS実装を変えた再試行は新評価である。
各API/CLIの時間/RSS境界と未観測encodeCount/cause/cleanupErrorはPLANとrawを参照する。

保存済み証拠のdata-only検証:

```sh
node docs/history/compiler-budget-max-quality-20261012/summarize.cjs --check
```

counts/一意性・保存hash/bytes・source/native/JS/fixture/license/environment・最高適合q60/より高いJPEG候補・既定とopt-inの分離・非公開/既存出力保護を確認する。
集計の再生成は引数なしのsummarize.cjsで行えるが、evidence-hashes.jsonには再生成後のsummaryを結び直す。
この確認だけのために画像処理や全benchmarkを再実行しない。
