# 評価記録の検証 (#887)

## 対象検証

2026-10-12、評価branch上で次を実行した。画像処理の再実行ではなく、保存済みデータと分類・集計・provenanceの検証である。

| command | exit | 結果 |
|---|---:|---|
| `node docs/history/target-bytes-operational-20261012/summarize.cjs --self-test` | 0 | E300、cause付きE123 timeout、cause未観測resource拒否、best-effort未達、既知誤拒否、measurement/safety failureの分類 |
| `node docs/history/target-bytes-operational-20261012/summarize.cjs --check` | 0 | 主108記録、239 file bindings、sourceごとの異なるnative、固定環境、保護チェックと決定的再集計の一致 |
| `git diff --check` | 0 | whitespace差分チェック |
| `git diff --cached --check` | 0 | 納品するstaged差分のwhitespaceチェック（raw configure logだけは下記の保存属性） |

`evidence-hashes.json`は無効batchを含む保存fileのbytes/hashを結ぶ。無効batchは採用集計へ入れない。
configure logに元からある2行の末尾空白は変更せず、history内の`.gitattributes`でその1 fileだけwhitespace検査対象から外した。
build失敗log2件の公開用コピーからhome/個人PATH/一意なCodex・local temporary pathを除去し、`LOG_REDACTIONS.json`に元bytes/hashと公開派生物bytes/hashを結んだ。原本はGit外のローカルに保持し、公開保存hashは派生物を対象とする。
独立したnative build、14 calibration/semantic対照、108主操作、6付帯チェックの実command・exit・sourceは保存証拠を参照する。
workerのexit 0は測定処理の完了であり、APIのE300/E123を製品成功へ変えない。CLIのexit 1も別に保存している。

## 失敗・不一致の扱い

- 共有Cargo targetの初回batchは異なるsourceで同じnativeがロードされたため無効。raw/集計/logを保持し、性能比較から除外した。
- 独立targetの初回buildはSDK/clang不一致によりexit 1。失敗logを保持し、既存Xcode SDK26.5/clangをcommand単位で選び直した。依存・製品・CI・global設定は変更していない。
- その後、各clean checkoutのcanonical `npm run build`がexit 0、native hashが異なり、既知q100対照で旧E300/新受入を確認してから有効batchを実行した。
- valid-input timeoutを期待拒否に加算せず、旧探索の既知誤拒否も同等成功の時間比較から除外した。旧累積metricsは外側時間の代わりにしない。

## レビュー

セルフレビューでは固定条件/分母、byte適合とtimeout、外側/native/RSS境界、既存出力保護、公開APIに存在する運用選択、SemVerと公開準備の判断を確認した。

resource境界と公開判断の専門レビューはSol mediumで1件実施した。初回はPLANの「一時source残存なし」が観測範囲を超えるP3としてBLOCK。
新output未公開・owned output parentのsibling stagingに限定し、OS tmp内のprivate source snapshotは独立全走査していないと明記した。
同じレビュアーによる指摘範囲だけの最終確認は**ALLOW**。EVALUATIONの観測境界と一致することを確認した。
この文書修正による画像再測定は行っていない。

Companionの初回Gateは公開logの個人path残存でBLOCK。この2 logの公開用コピーだけをredactし、保存hashと説明を更新した。画像処理・buildの再実行はしない。
Codex Companionの指摘範囲の最終Gateとhosted検証は、納品時の最終HEADに結び付けてPR/Issueに記録する。
過去の#885 Gate/CIを今回の成功証拠として扱わない。
