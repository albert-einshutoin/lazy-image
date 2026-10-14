# #906 decode切り分け計画

2026-10-14。#902/#904はCLOSED、#703はOPENを維持。base `c0e5445b3c9c07fe5d0f6624c00c51b9c442e1aa`。旧historyの661ファイルは[source-manifest](source-manifest.json)へbytes/SHA-256とGit subtreeを固定し、開始/納品時に照合する。今回の実行は別task-local directoryで行い、旧証拠と測定を上書きしない。

## 対象と仮説

旧quality.jsonは`metadata-1 A` / `desktop-A/images/metadata-1/image-240.webp`、旧quality-fresh.jsonは`coffee-64 Original` / `reference/coffee-64.png`。aggregate EncodingError、complete=true、natural寸法0。旧collectorはPromise.allの最初のエラーのみで、個別promise結果はない。Resource Timingの0/欠落だけではHTTP失敗/空body/メモリ不足を証明できない。

画像bytes/形式、HTTPのURL/body/中断/cache、ページの画像数/観測/操作の3層を分ける。まず保存済み同一bytesを使い、公開1.5.0の画像を再生成しない。既存Sharp0.35.4の全pixel decodeは画像診断で、compiler再実行やChrome成功ではない。

## 固定初期matrix

[matrix.json](matrix.json)の6条件×3trial、計18 page load。順序はtrialごとに単体WebP、単体PNG、metadata triplet、coffee-64 triplet、旧sheet、旧fresh sheet。各loadは新queryを付けるがasset URLは同じで、cold cacheとは呼ばない。cacheを消去せずResource Timingと操作履歴を保存する。

- Desktop viewport1440×1000、DPR1、browser zoom100%。実DOMと稼働browser versionを確認。
- 旧sheet/旧triplet/qa.jsはbyte不変で、保存previewを新しいloopback配信directoryへコピーする。
- 単体ページだけを新規作成し、対象画像と旧qa.jsを使用。
- コマンド: `python3 -m http.server 8877 --bind 127.0.0.1 --directory <NEW_PHASE>/preview`。
- CUAから`#qa-observation`の実browser内decode結果を読み取り、新規rawへ保存。loadingをJSON化しない。観測待機は各30秒まで、未完了はpendingとして保存する。
- 18読み込みの概算3〜5分。新しい生成/build/benchmarkは不要。

## 診断・修正の受入

新しい診断collectorは個別decode成功/error/timeout、イベント、URL/寸法、実viewport/DPRを保存し、元qa.jsとは別ファイルにする。timeoutはnative decode強制停止ではなく観測上限で、後の完了を保存timeoutへ上書きしない。src変更は成功と扱わない。

旧collectorの集約では検出できない複数失敗/pendingの対象テストから始める。実browserの制御された破損画像を混ぜた対照で、正常画像を失敗へ巻き込まず、破損を成功へ変換しないことを確認する。制御された失敗は過去2件の再現と呼ばない。

HTTP診断fetchのstatus/body SHA-256とBlob native decodeは独立した取得経路で、元imgリクエストのbody同一性を直接証明しない。追加条件は初期結果で残る仮説だけに限り、画像数/cache/decode順序等を同時に変更しない。

## 終了条件

2件それぞれを原因確認/限定条件で再現/現在再現せず当時未確定/必須証拠不足へ分類する。未再現だけで完了せず、画像・取得・ページの必要確認、取得できない当時証拠と再開条件を示す。未実施の必須確認がある間はGoal完了にしない。

必要最小helperと記録を同Issueの1PRへ納品。対象検証、セルフレビュー、Companion Gate、関連する最終HEAD CI、保護merge、同merge SHAのmain成果物/CI/Securityを確認する。旧artifactの変更、製品/API/codec/依存/CI変更、fallback、timeout緩和、全画像再生成、release/deploy、branch削除、次Issueへの着手はない。
