# 検証・レビュー記録 (#885)

対象検証の実ログと終了状態はEVALUATION.mdとchecks/に保持した。
セルフレビューでnormal-only defaults移動、quality/chroma/progression/smoothingの最終適用、fast modeの非変更、
panic/error/metadata/budget共有経路の非変更、DQTの実出力検査と測定の分母・未達・scopeを照合した。

- Rust専門レビュー: Sol (`gpt-5.6-sol`) medium、`/root/rust_jpeg_review`。base9086814からHEADd66c451のencoder変更、
  ICC equality assert、直接caller、locked mozjpeg sourceと保存ログを読み取りレビュー。ALLOW、関連指摘なし。
- JS専門レビュー: Sol (`gpt-5.6-sol`) medium、`/root/js_jpeg_contract_review`。artifact-compiler.test.jsの10行差分のみ。
  ALLOW、指摘なし。WebP検証維持、JPEGのpolicy/artifactId/E300/processingとoutputDir不存在を確認。
- 二つのレビューは独立した担当ファイルへ限定し、編集・再測定・CI実行は行っていない。

完全検証とCompanion Stop Review Gateは、この記録作成時点では未実施。未実施を成功へ数えない。

## 関連指摘範囲の最終レビュー

完全検証の最初の`npm test`はJPEG golden pixel hashでexit1となった。
旧bindingのhash再現と修正後の画質/容量を確認し、JPEG hash更新と相対5%容量優位assertの削除だけを行った。
旧5%期待は未達として保持し、public targetBytes/strict条件や画質閾値は削除・緩和していない。
この追加2ファイルと直接証拠を同じJS reviewer/Sol mediumが限定最終レビューし、ALLOW・指摘なし。
Rust製品経路は専門レビュー後に変更していない。
対象goldenは6成功/exit0。初回完全検証のfailureとその後の対象成功を別記録に保持した。

Companionとhosted CI/merge/Issue完了の最終証拠はIssue #885の完了報告を正本とする。
本記録はそれらの成功を先取りする証拠には使わない。
