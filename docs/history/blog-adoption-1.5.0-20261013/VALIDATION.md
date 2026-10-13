# 対象確認・セルフレビュー

- 公開install/prepare/6build/negative recorder/audit/preview再現の実exitを保存。正常18/18はA/Bそれぞれの実rawを集計し、期待負例を加算していない。
- 6ffda40のbuild/measure/prepare bytesは、実測consumerと納品consumerで完全一致。SDKのindex.js/index.mjs/artifact-policy/artifact-compiler/.nodeはidentityのhashと測定後も一致した。
- 保存matrixは正確なA1/A2/A3/B1/B2/B3と6input ID、source sha256、compiler version1.5.0を確認。132実画像filesのformat/dimensions/bytes/hash、48 budget/range、ICC/EXIF/XMP、全trialの画像hash一致を確認した。source metadataのGPS IFD pointer存在も保存。
- HTML pure self-testは64px実manifestのURL/srcset/sizes、placeholder、WebP sourceとJPEG fallbackを確認。実6画像フロー、個別3負例、既存29files不変、owned images親内のstagingなしはruntimeで確認。
- browser5表示の各6画像decode/currentSrc、12 image/placeholder Resource Timingのbodyと実files、HTML/srcset/manifest全参照HTTP200を照合。品質の最終6tripletは各3画像の同じwidth/height・decode成功を確認し、全6画面をAgentが目視した。2回の大比較sheet失敗は残した。
- 修正した再現helperは主buildを再測定せず、新previewを生成して全observed記事と最終quality-singleの実HTML/画像/qa.js bytesが観測済みpreviewと一致することを確認。最初に実行した補助codeはexecuted-auxiliary snapshot/hashで区別する。
- consumer8 JS filesのnode --check、公開derivative/原本のledger hash・private path/token scan、git diff --checkを行う。scan初回は検査codeの文字列'/Volumes/'を誤検出し、実volume pathに限定して再確認した。実private pathを除外していない。consumer LICENSEの行末空行は公開derivativeで正規化し、原本MIT hashをledger/identityとroot LICENSEに保持した。

セルフレビューは公開SDKと評価code、固定入力/予算と結果、module後/whole-process時間、生成容量/実取得容量、通常/拒否、正常記事/比較診断、主測定/補助code更新、原本/公開derivativeを分けて確認した。未観測native totalMs、XMP付き入力challenge、OS全tmp、細線保持、18画像同時比較の失敗原因を成功へ昇格していない。

Companion GateとPRの最終HEAD・CI/merge対応はIssue #902 / 対応PRへ記録する。本体/公開API/CI変更はなく、文書完了確認だけの全build/全benchmark/公開release matrixの再実行は追加しない。
