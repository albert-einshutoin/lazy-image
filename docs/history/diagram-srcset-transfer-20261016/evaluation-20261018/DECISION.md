# 判断

#904の**SVG標準採用を維持する**。raster必須の場合は、実manifestから684/960/1920wを組み立て、CSS slotとsizesを一致させる既存方針を維持する。保存済み同一WebPの今回のChrome初回24 loadではMobile DPR1/2とDesktop DPR1で画像body削減を観測し、Desktop DPR2では削減なしだった。

ブラウザが必ず最小候補を選ぶとは説明しない。取得済み1920pxの再利用を確認した一方、全3warm trialで684px追加取得を伴った。運用観測はcurrentSrcだけでなく、候補全entry・server・cache/SWを合わせる。追加要求・削減なしも結果として保持する。公開HTMLへforce-smallest、srcset除去、decode/fetch probeを追加しない。既存static-site exampleのmanifest/sizes/CSSはこの契約と一致しており変更しない。

この評価は同じ図と保存bytes、Chrome desktop emulation、loopbackの3trial/条件に限定する。速度/LCP/CDN効果、全browser/実機phone、最適品質・最適サイズ、compiler改善を主張しない。#906の旧decode原因unknownは変更せず、#902/#904/#906を再開しない。#703はOPENを維持し、#908の納品と取り込み後検証が揃ってから本Issue/Goalを完了する。
