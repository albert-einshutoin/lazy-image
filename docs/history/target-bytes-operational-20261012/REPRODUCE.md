# 再実行手順と前提

Node24.2.0/Apple M4/Darwin arm64での研究用labであり、productionや課金endpointへ接続しない。
既存fixtureは再生成しない。`fixtures.json`のimage/license hashを検証して使用する。

1. source1962f233…と21a15bc…の隔離clean checkoutを用意する。現mainを巻き戻さない。
2. 各checkoutでnpm ciを行い、lockの@napi-rs/cli3.9.0を確認する。
3. 独立したCargo targetを使いnpm run build。共有targetで同一crate名の成果物を使い回さない。
   この端末ではXcodeの既存clang/SDK26.5を各commandのDEVELOPER_DIR/SDKROOT/CC/CXXだけで選択した。
   正確なpaths/commands/exit/source/nativeはevidence/matched-sdk-build-results.jsonとbuild logsを参照。
   グローバルxcode-select・依存・製品・CIは変更していない。
4. evaluate.cjsのVARIANTSとDATA/TEMP pathsを新しいowned directoryへ明示的に変える場合は、runner hashも新しい評価として扱う。
   既存raw/matrixは上書きしない。matched-sdk-build-results.jsonへ実際の新build結果を保存する。
5. 以下で予算値とsemantic対照を凍結し、source/binary差と旧E300・新q100受入を確認してから主測定する。

```sh
node docs/history/target-bytes-operational-20261012/evaluate.cjs --calibrate
node docs/history/target-bytes-operational-20261012/evaluate.cjs --run
```

calibration/対照14操作、18条件×3trial×2source＝108主操作、付帯6操作。workerは既存isolatedCaseの
600秒guard、CLI childには120秒measurement guardがある。これらは製品の5秒timeoutではなく、
signal/worker failureはmeasurement failureとして別に保持する。未知・失敗trialをrepair/replayして成功へ変えない。

APIはmodule/fixture load後、constructorからpublish/拒否までの外側時間を計測。CLIはNode起動からexitまで。
API/compilerは10ms sampled whole-process RSS。CLIはpsのowned child PID RSS（KiB→bytes）、100ms sampling。
CLI native metrics/cause/cleanupErrorはpublic diagnosticsが露出しないので未観測とする。compilerのmetrics tapは
public toFileWithMetrics/toFileTargetBytesを同引数で委譲し、返却値を変えずに記録する。
API promise rejectionとworker exit0、CLI exit1は別の結果である。record.executionの実command/時刻を参照。

保存済み証拠のdata-only確認は次の対象検証で行える。

```sh
node docs/history/target-bytes-operational-20261012/summarize.cjs --self-test
node docs/history/target-bytes-operational-20261012/summarize.cjs --check
```

後者は108行/18条件/3trial・全file hash/bytes・source/native/build/lock/SDK/環境・matrix/runner・分類/再集計・
既存出力保護を確認する。無効batchは保存hashだけを確認し、採用集計へ混ぜない。
対象検証のために全build/全benchmark、画像再測定を繰り返す必要はない。
