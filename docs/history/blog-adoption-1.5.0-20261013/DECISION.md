# 採用判断 (#902)

**条件付き採用。推奨はB：容量budgetと明示maxQuality付き。**
固定した6画像の静的技術ブログでは正常18/18、48 budget全適合、全page process最大2.113秒で60秒基準内。mobile/desktopの全画像とJPEG診断がdecodeし、manifest参照、no-upscale、Orientation/metadata、負例保護を満たした。写真・通常の文字図は記事の表示寸法で実用になるとAgentが判断した。

一方、大入力の極細gridは弱まる。細線を完全に保持する図まで合格とはせず、18画像同時比較sheetの2回のdecode失敗も未解消として保存した。正常6画像ページの表示成功と混同しない。利用者が自分の原図で表示寸法と必要な細部を確認し、同じpolicyの容量・受入を確認することが採用条件。実利用者が採用した証拠やユーザー本人の画質承認ではない。

## Bを勧める理由と効果

BはWebP max80、JPEG max85で、今回のA既定qualityと同じ。A/B画像は全trialで同一bytes/hash、追加の圧縮改善は得ていない。代わりに同じ品質域でstrictなbyte上限を明文化し、verified manifestへ対応付けられる。3trialで処理コストが大幅に増える観測はなく、中央値差を速度優位とはしない。

推奨budgetはPLANの固定値：WebP320/640/960に40/100/200 KiB、JPEGに60/150/300 KiB、各maxQuality80/85、placeholderあり。240/64pxは320枠の同じ予算を実幅へ指定。存在しないbudget targetやcompiler未提供のminQuality/effort/timeoutを案内しない。現consumerのfixture用prepareを、任意入力のOrientationや幅を安全に推測する一般処理とは扱わない。

予算管理が不要ならAもこの固定用途で成立するが、Bより成果物上限を明示しにくい。Bの利点は今回の画像bytes削減ではなく、capacity条件を失敗として検出できる点。厳しくすれば常に小さく速くなるという判断ではない。

6画像のアプリ本体は公開API→実manifest→HTMLの小さな処理で済み、画像privacy/検証と画像一式の確定をlibraryに任せられた。全variant/placeholderは296,658 B、browser実取得はdesktop97,562 B・mobile56,088 Bで、必要なvariantだけを配信する効果を確認した。これを「元画像を配った記事との実測速度差」や一般codec圧縮率とは呼ばない。manifestとstrict上限を必要とするbuildには導入作業に見合う効果があるが、全サイト更新/失敗rollbackやCDNはアプリの責任に残る。

## 利用者が確認する条件・次の1段階

自分の写真・図について、要求幅と正規化実幅、非alpha JPEG fallback、Orientation/ICC、文字・線・色、DPR、byte予算とE123、実job時間、失敗時の既存サイト保護を確認する。maxQualityは知覚品質や時間/受入保証ではなく、5秒は処理境界検査でcodec強制停止/SLAではない。

**最優先の1件は、細線を情報として使う実際の技術図で必要な出力幅を確認すること。** 固定した960pxで失われる細部を明確にし、その画像種に必要な表示/運用policyを決める。今回の結果から製品コード改修を開始する根拠は確定していない。本Goalでは追加改修・次Issue・新versionへ進まない。
