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

## 完全検証で発見した既存テスト条件とsetup

2回目の完全`npm test`はexit1。既存ignored `npm/darwin-arm64/lazy-image.darwin-arm64.node`（2025年生成、約5.6MB）が
release guardの『stale npm』拒否を先に発動し、単体テストの意図したmissing-artifact拒否まで到達しなかった。
生成物を削除せず一時退避すると、既存release-artifacts単体テストは成功した。CI/scriptの変更は行っていない。
このsetup failureを[実結果](checks/full-local-setup-results.json)と[ログ](checks/full-npm-test-setup.log)に保持した。
退避復元のvolume間renameはEXDEVとなったが、標準mvで復元し、元のSHA-256
`8649e8a9bd3f7a0aec5a495c1a198349bcc61dd5286463d36738f0da8f2a69cd` と一致を確認した。

setupを分離した次の完全`npm test`はRust cache testでexit101
([実結果](checks/full-local-cache-results.json)、[ログ](checks/full-npm-test-cache.log))。
`encode_prepared_can_run_multiple_times_against_same_cache`が、64×64単色JPEGのquality上昇で
サイズ472→346 Bとなることを失敗扱いしていた。サイズ単調性は未保証で、cache再利用契約ではない。
製品経路へ追加patchを重ねず、この誤条件から共有の観測可能な契約へclean breakした。
quality50→80→100→50で各JPEGをdecodeし64×64を検査、異なるqualityを挟んだq50出力bytesの完全一致を検査する。
新閾値・workaround・legacyを追加していない。対象再検証1成功/exit0 ([ログ](checks/green-cache.log))。
同じRust reviewer/Sol mediumがこのtest-only差分を限定最終レビューしALLOW・関連findingなし。

完全native buildはsource365b33eでexit0。binary SHAは修正後測定と同じ
`10497403b2fd60c1f01e929fbbe8a3c6dabb5d448d9927a15d99e771d2385174`。
その後の変更はgolden data・test-only module・記録だけで、encoder製品部分・依存・APIは変更していない。
成功済みbuildを繰り返さず、完全testとcompiler/quality証拠の未完了部分を検証する。
