# 検証・レビュー記録 (#885)

対象検証の実ログと終了状態はEVALUATION.mdとchecks/に保持した。
セルフレビューでnormal-only defaults移動、quality/chroma/progression/smoothingの最終適用、fast modeの非変更、
panic/error/metadata/budget共有経路の非変更、DQTの実出力検査と測定の分母・未達・scopeを照合した。

- Rust専門レビュー: Sol (`gpt-5.6-sol`) medium、`/root/rust_jpeg_review`。base9086814からHEADd66c451のencoder変更、
  ICC equality assert、直接caller、locked mozjpeg sourceと保存ログを読み取りレビュー。ALLOW、関連指摘なし。
- JS専門レビュー: Sol (`gpt-5.6-sol`) medium、`/root/js_jpeg_contract_review`。artifact-compiler.test.jsの10行差分のみ。
  ALLOW、指摘なし。WebP検証維持、JPEGのpolicy/artifactId/E300/processingとoutputDir不存在を確認。
- 二つのレビューは独立した担当ファイルへ限定し、編集・再測定・CI実行は行っていない。

初版時点では完全検証とCompanion Stop Review Gateは未実施だった。以下の後続実結果を、対象検証の成功と分離して記録する。

## 関連指摘範囲の最終レビュー

完全検証の最初の`npm test`はJPEG golden pixel hashでexit1となった。
旧bindingのhash再現と修正後の画質/容量を確認し、JPEG hash更新と相対5%容量優位assertの削除だけを行った。
旧5%期待は未達として保持し、public targetBytes/strict条件や画質閾値は削除・緩和していない。
この追加2ファイルと直接証拠を同じJS reviewer/Sol mediumが限定最終レビューし、ALLOW・指摘なし。
この段階ではRust製品経路は専門レビュー後に変更していなかった（後続の共有探索修正は末尾に別記）。
対象goldenは6成功/exit0。初回完全検証のfailureとその後の対象成功を別記録に保持した。

Companionとhosted CI/merge/Issue完了の最終証拠はIssue #885の完了報告を正本とする。
本記録はそれらの成功を先取りする証拠には使わない。

## 完全検証で発見した既存テスト条件とsetup

2回目の完全`npm test`はexit1。既存ignored `npm/darwin-arm64/lazy-image.darwin-arm64.node`（2025年生成、約5.6MB）が
release guardの『stale npm』拒否を先に発動し、単体テストの意図したmissing-artifact拒否まで到達しなかった。
生成物を削除せず一時退避すると、既存release-artifacts単体テストは成功した。CI/scriptの変更は行っていない。
このsetup failureを[実結果](checks/full-local-setup-results.json)と[ログ](checks/final-npm-test.log)に保持した。
退避復元のvolume間renameはEXDEVとなったが、標準mvで復元し、元のSHA-256
`8649e8a9bd3f7a0aec5a495c1a198349bcc61dd5286463d36738f0da8f2a69cd` と一致を確認した。

setupを分離した次の完全`npm test`はRust cache testでexit101
([実結果](checks/full-local-cache-results.json)、[ログ](checks/final-npm-test-clean-setup.log))。
`encode_prepared_can_run_multiple_times_against_same_cache`が、64×64単色JPEGのquality上昇で
サイズ472→346 Bとなることを失敗扱いしていた。サイズ単調性は未保証で、cache再利用契約ではない。
製品経路へ追加patchを重ねず、この誤条件から共有の観測可能な契約へclean breakした。
quality50→80→100→50で各JPEGをdecodeし64×64を検査、異なるqualityを挟んだq50出力bytesの完全一致を検査する。
新閾値・workaround・legacyを追加していない。対象再検証1成功/exit0 ([ログ](checks/green-cache.log))。
同じRust reviewer/Sol mediumがこのtest-only差分を限定最終レビューしALLOW・関連findingなし。

完全native buildはsource365b33eでexit0。binary SHAは修正後測定と同じ
`10497403b2fd60c1f01e929fbbe8a3c6dabb5d448d9927a15d99e771d2385174`。
source240d23eまでの変更はgolden data・test-only module・記録だけで、encoder製品部分・依存・APIは変更していなかった。
成功済みbuildを繰り返さず、完全testとcompiler/quality証拠の未完了部分を検証する。

## 共有探索修正前のローカル完全検証

[最終実結果](checks/final-verified-local-results.json) のsourceは
`240d23e061af209d0c34b12feaf93721319a2e05`、native bindingは上記10497403…5174。
`npm test` **exit0** ([ログ](checks/verified-npm-test.log))、
`npm run test:bench:release` **exit0** ([ログ](checks/full-bench-release.log))。
既存Rust ignored testはログどおり保持し、実行成功へ加算しない。
ignored npm生成物は検証中だけ退避し、終了後に元のhashへ復元した。後者の実結果にもrestored=trueを記録。

[compiler](raw/full-validation/compile-image-corpus.json)、[quality](raw/full-validation/quality-matched.json)、
[集約](raw/full-validation/release-evidence.json) は新規ローカル証拠で、native1.4.3/source240d23e。
Markdownのv1.3.0表題はrunnerの固定文字列で、測定版の証拠にはしない。
正常受入13/13（39trial）、strict budget11/11、期待拒否2/2（6trial）を分離。
公開manifest/checksum、private metadata除去、該当metadata fixtureのICC bytes保持を検査した。
処理・検証失敗0。codec39比較の品質達成はJPEG13/13、WebP13/13、AVIF11/13。
AVIFのalpha-1/alpha-2品質未達2/39、tiny transparent-avifのJPEG/AVIF容量未達2/39を保持する。
集約passed:trueはこれらの全目標達成を意味しない。

全corpusは修正後のみの追加受入検証であり、修正前の全13入力を同条件測定した比較ではない。
同条件の前後比較はEVALUATION.mdの3 JPEG入力に限定する。
この追加記録でも同3入力の候補bytes/hash/SSIMは保存済み修正後と一致した。
実コマンド、hash、分母、未達はcomparison.jsonから各raw/checkへ辿れる。

## PR #886の共有探索指摘に対する追加検証

[自動review指摘](https://github.com/albert-einshutoin/lazy-image/pull/886#discussion_r4240418811) は実際の契約違反と確認した。
追加のRust regressionは64×64/q100=346 Bを直接encodeして適合を証明しても旧検索がE300となりexit101。
native追加testも旧bindingでexit1。新source64a2ac5のcanonical buildはexit0、native digestは2ce79f28…37e9。
`cargo test --no-default-features target_bytes` は4成功/exit0。
`node test/integration/target-bytes-file-native.test.js` はnormal/fast、Buffer/fileのmax候補一致を含みexit0。
compilerとbasic（50test）もexit0。各実ログと小fixtureのAPIコストはEVALUATION.md末尾から辿れる。
source21a15bcのGate/hosted結果は、追加した共有探索コードの承認・検証へ流用しない。

追加した共有探索・回帰を同じRust/JS専門reviewer（Sol medium）が担当範囲を分けて限定レビューした。
Rustは降順の最高適合quality、全候補不適合時の最小bytes/高quality tie、strict拒否、permit/prepare再利用とError伝播を確認。
JSはnormal/fast、Buffer/fileの直接q100 oracle、generated型署名の維持、source/native/fixture照合とCPU増加の明記を確認。
両方ALLOW、関連指摘なし。新経路の完全検証・Companion・hosted成功はこの専門レビューに含まれない。
