# #906 decode切り分け結果

2026-10-14。**対象2件は今回再現せず、当時の原因は未確定。** 画像・取得・比較ページの各層を調査した結果で、過去のdecode失敗が解消したという結論ではない。#902/#904の証拠を変更せず、新Issue #906へ記録する。

## 2件ごとの判定

| 旧観測 | 保存bytesと今回の結果 | 原因判定 |
|---|---|---|
| quality.json / metadata-1 A | WebP240×320、3062 B、SHA-256 `e28df97d900d85c7a7aef5a04c599e7041f619163560b852d00104ceceee5b51`。Sharp全pixel、Chrome単体/triplet/sheet、独立HTTP body/hash/Blobが成功 | 現存bytesの恒常的decode不適合は再現せず。当時の取得body/中断/操作状態がなく未確定 |
| quality-fresh.json / coffee-64 Original | PNG64×43、6293 B、SHA-256 `0face9591e51f0ee8342ba48f047c1881e03e5c4ba1e59477129e4f9c1ed6d8d`。同じ各層の確認が成功 | 同じく現在再現せず、当時原因未確定 |

既存Sharp0.35.4の全pixel raw decodeを[image-check](image-check.json)へ保存した。metadata取得だけの成功ではない。browserは稼働Chrome154.0.8037.98、1440×1000/DPR1、Cmd+0でzoom100%へresetした。[環境](environment.json)を参照。公開1.5.0で作った旧成果物を検査し、compiler/SDKを再実行していない。

## 初期matrixと診断

旧HTML/qa.jsと同一bytesの単体2、triplet2、18画像sheet2を各3trial。18/18 load、132 image.decode promiseが正常完了し、natural寸法0/aggregate error/pendingは0。HTML queryだけを変え、asset URLと既存cacheは再利用している。cold cache試験ではない。

新しい個別collectorを付けたsheet2×3trialも108/108成功。各sheetの2対象について独立no-store fetchのstatus200/body SHA-256一致、Blob native decodeが12/12成功。これは**別fetchの結果**で、過去や元のimgリクエストのbodyを直接取得した証拠ではない。現在のURLと保存fileの一致は確認できるが、当時のHTTPbody/中断/cache/historyを復元できない。

18枚の並列decodeがこの条件で必ず失敗する、あるいは恒常的なURL誤りがあるという仮説は今回再現しなかった。画像数、browser全般、過去の一時的失敗やメモリ不足を原因として断定しない。browserのRSS/peak memoryは測定しておらず、画像寸法から総memoryを推測しない。

## 確認した観測上の不足と修正

旧qa.jsはPromise.allの最初のエラーだけを保存しており、複数失敗/pendingの個別結果を保存できない。新しい[diagnose.js](diagnose.js)では全画像の成功/error/timeout/unknownを独立に記録。src変更やnatural寸法の不整合はunknown、timeout後のlate successは元のtimeoutを書き換えない。描画fallbackや成功へのcoerceはない。旧qa.jsは無改変。

初期診断scriptをbody末尾へ置くと、早いload/errorが登録前に終わり、9記録中4記録でイベント0件だった。[初期raw/HTML](evidence/early-listener/)を保持し、parserが画像を作る前へ登録するようprepareを修正した。修正後は全9 loadで、全117 DOM画像のload/errorイベントと個別decode結果が一致した。この観測欠落の修正は、過去EncodingErrorの原因を説明した証拠ではない。

## 対照の結果

| 対照 | DOM decode | 独立HTTP / Blob | 意味 |
|---|---|---|---|
| PNGを64 Bへ切断 | 3画像すべて成功 | 切断bodyも200/hash一致/Blob成功 | このChromeは切断PNGを受け入れた。拒否fixtureとして不十分で、結果を保持。完全な画素/コンテナの正しさは未判定 |
| 不存在URL | 正常2成功、該当1 EncodingError | 404、Blobは実行せず | 取得失敗を画像ごとに区別 |
| 明確な非画像36 BをPNG拡張子で配信 | 正常2成功、該当1 EncodingError | 200/hash一致でもBlob EncodingError | HTTP200/hash一致だけは画像decode成功を保証しない |

これらは制御された対照で、過去2件の再現ではない。正常画像を失敗へ巻き込まず、失敗をfallbackで成功へ置換していない。

## 証拠・実行境界

[raw](evidence/browser/)の最終27 loadは初期18＋診断sheet6＋対照3。初期診断9と寸法validatorの補助失敗1は別保存。初回validatorがJSON key順を比較して正しいviewportを誤拒否したため、数値比較へ修正した。native decodeはその試行も成功しており、画像失敗には加算しない。

旧661ファイル/21,408,725 Bは[source-manifest](source-manifest.json)で固定して再照合。baseline collector/HTMLのfreeze commitは`9762124a7e04da8bfcd1b1e5c95461b71b060b84`、最終diagnostic prepareは`e1bfcf508fc3adab080dfe6049c168f9da23ce95`。observer/collectorとHTMLの実hashも[binding](evidence/diagnostic-binding.json)に保存。納品HEADとは分ける。原本と公開copyは[LOG_REDACTIONS](LOG_REDACTIONS.json)。

対象テストは旧collectorでexit1、新collectorでexit0（5契約）、prepareチェックexit0（旧HTML不変、登録順、既存先拒否）。Node fixture timeoutは短いテスト時計であり、実browserのtimeoutを観測した結果ではない。再実行と受入検証は[REPRODUCE](REPRODUCE.md)。

## 再開条件

同じ保存hashでEncodingError/自然寸法0が再発したら、今回collectorの個別結果・request URL・実HTTP応答/中断・load/error・src/navigation変更・browser versionを同時保存する。原本request bodyを取得できなければその同一性はunknown。新しい根拠が得られるまで製品/codecの修正や環境総当たりへ進まない。今回の調査完了を過去不具合の解消、全browser受入、SLAとして扱わない。
