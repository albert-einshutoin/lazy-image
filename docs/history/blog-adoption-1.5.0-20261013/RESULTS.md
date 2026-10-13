# 実測結果 (#902)

対象は公開npm Native 1.5.0（公開source `023a8c9368eef58f17ecc6f412363d24e5aa9b67`）。consumer測定コードはfreeze commit `6ffda40cd1af311711d17c178e838ad842f1d64f` と各rawのcode hash。macOS27.0 / Apple M4 10 CPU / 32 GiB / arm64 / Node24.2.0 / npm11.3.0。公開registryへsave-exact install、lockと実JS/.nodeの所在/hashは[identity](evidence/identity.json)、inputsとpolicyは[PLAN](PLAN.md)・[consumer](consumer/)に対応する。npm link・local tarball・checkout SDK・binding overrideは使用していない。

## 正常系とbuild時間

A/B各3回、計36 compileをfresh process・concurrency1で実行。全6ページがprocess exit0、A/Bとも正常入力18/18成功、E123を含む正常入力の失敗は0。初回を除外していない。OS/filesystem cacheや他processの影響は制御しておらず、アプリ側cacheは使っていない。

| 条件 | process wall個別値 ms | 中央値 / 最大値 ms | page wall中央値 / 最大値 ms | 生成全files容量 |
|---|---|---|---|---|
| A budgetなし | 2164.4, 2005.3, 1924.1 | 2005.3 / 2164.4 | 1963.7 / 2105.7 | 毎回314,004 B |
| B budget＋上限 | 2112.8, 1934.5, 1947.6 | 1947.6 / 2112.8 | 1909.4 / 2068.1 | 毎回314,328 B |

process wallはNode起動〜終了、page wallはmodule load後のsite作成〜HTML作成。出力監査・browser・参照画像作成は測定外。Bの各回60秒基準は満たした。3trialの57.7msの中央値差からBの高速化を主張しない。p95/p99/SLAは未測定。公開compileImage manifestにnative metricsがないため、native totalMsはnull/未取得で、外側時間と同一視しない。

| 画像 | A wall中央値 / 最大値 ms | B wall中央値 / 最大値 ms |
|---|---|---|
| coffee | 75.4 / 94.6 | 73.3 / 76.8 |
| chelsea | 57.8 / 59.7 | 58.4 / 59.9 |
| ui-1 | 41.9 / 42.0 | 42.7 / 45.3 |
| metadata-1 | 45.7 / 46.0 | 45.7 / 45.8 |
| large | 1717.4 / 1830.8 | 1664.4 / 1808.7 |
| coffee-64 | 29.4 / 30.4 | 28.2 / 29.2 |

[全個別値](evidence/runs/summary.json)、[全process](evidence/runs/processes.json)、個別raw/log/実exit/全manifestと生成物を保存した。大入力のcompileがpage時間の主要部分だが、decode/resize/encode内訳や探索回数は公開APIで観測していない。

## 出力quality・budget・容量差

Bの16 variant×3回＝48 budget全件適合。JPEG全件q85、WebP全件q80で、それぞれ明示上限内。A/B・全3trialのvariant/placeholderは同じbytes/hash。今回の余裕あるbudgetでは追加の画像容量削減を得ていない。Bの生成容量+324 BはmanifestのtargetBytes情報によるもので、画像の増加ではない。全16variantと6placeholderは合計296,658 B、全29files容量はHTML/manifestも含む。

| 画像・幅 | JPEG bytes（q85） | WebP bytes（q80） | B目標JPEG / WebP bytes |
|---|---:|---:|---:|
| coffee320 | 16,069 | 13,736 | 61,440 / 40,960 |
| chelsea320 | 15,747 | 13,022 | 61,440 / 40,960 |
| ui-1 320 | 5,823 | 2,788 | 61,440 / 40,960 |
| metadata-1 240 | 6,826 | 3,062 | 61,440 / 40,960 |
| large320 | 7,989 | 5,170 | 61,440 / 40,960 |
| large640 | 31,495 | 21,812 | 153,600 / 102,400 |
| large960 | 86,683 | 63,286 | 307,200 / 204,800 |
| coffee-64 | 1,482 | 1,116 | 61,440 / 40,960 |

各画像hash、format、寸法、quality、targetBytes、ICC outcomeはmanifestと[132実ファイル監査](evidence/runs/summary.json)にある。要求640/960が落ちる写真/UIや、240/64pxに対して存在しないbudgetを指定していない。placeholderはbudget対象外で既存strip-all契約を維持する。

## 実ブラウザの取得と表示

既存Chrome **154.0.8037.98**、desktop1440×1000、mobile相当390×844、DPRは両方1。mobile UA/device実機の評価ではなくviewport評価。temporary viewportは確認後resetした。CLIで起動する別Chrome155や過去releaseのChrome観測へ読み替えない。

| 表示 | 実取得画像body B（placeholder込み） | 主画像 / placeholder |
|---|---:|---|
| desktop A / B | 各97,562 | WebP6 / WebP6、全decode成功 |
| mobile A / B | 各56,088 | WebP6 / WebP6、全decode成功 |
| JPEG-only診断 | 133,182 | JPEG6 / WebP6、全decode成功 |

Resource TimingのencodedBodySize/decodedBodySize・transferSize・responseStatusを[個別browser JSON](evidence/browser/)へ保存。body値はHTTP header/転送全体とは別。全29生成files容量とも別。mobileのlargeはimage-640.webpを342×342 CSSで表示し、desktopは960pxを選択。DOM naturalWidthはdensity補正後の値で、mobile342をencoded幅と呼ばない。実encoded幅640はcurrentSrcとfile検査で対応する。smallは64×43のままでsrcset/sizesも64px。metadataはencoded320×240 / Orientation6から240×320へ正規化され、参照の方向と一致した。

[HTTP/body照合](evidence/browser/http-and-body-check.json)では選択画像・placeholderのbody bytesを実ファイルと照合し、HTML/srcset/JPEG/placeholder/manifest全参照が200・同bytes。JPEG fallbackの参照decodeはsourceを除いた測定外診断で確認し、WebP対応browserの実fallback negotiationとは呼ばない。主測定のHTML/画像を変更していない。

## metadata・画質

metadata-1原入力にはEXIF/Orientation6、GPS IFD pointer、ICCがある。XMPはない。全132出力でEXIF/XMPなし、GPSはEXIF除去に含まれる。Chelsea/metadataの安全なICCはvariantでpreserved、placeholderではstrip、他variantはabsentで、実container存在とmanifest outcomeが一致した。新しいXMP付き入力の削除challengeは今回実施しておらず、出力XMP不在の観測をその実証とは呼ばない。

原画像/A/Bを同じwidth/heightで表示し、6入力の各3画像がdecodeした最終[個別品質JSON・画面](evidence/browser/)を保存した。大入力は960×960で表示した中心320px crop、他は各実出力寸法。metadataの参照だけ公開ImageEngine.autoOrient(true)→PNGで方向を正規化し、測定外・最適化fallbackではない。

- coffee：カップ/皿の輪郭、液面、木目・ハイライト・色を判別できる。候補では原図の微細な木目が少し平滑化されるが、記事中の写真として支障を認めなかった。
- chelsea：眼・鼻・ひげの輪郭と毛の方向・色は保たれる。細かい毛は少し柔らかい。A/B差は見えず、hashも同一。
- ui-1：3行の文字と3本の水平細線を読め、線の欠落や文字の潰れは認めなかった。
- metadata：右側の青帯、回転した文字/線が参照PNGと同じ方向。正常ページでもportrait寸法を確認。
- large：グラデーション・円/斜線の構図は保つが、原図の極細gridが候補で弱まる。resizeを含む全pipelineの差で、encoderだけの原因とは断定しない。細線そのものを情報として残す用途の合格とはしない。
- coffee-64：小さなカップとして認識でき、拡大していない。元の600px写真と同等の細部を主張しない。

これはAgentの目視で、利用者本人の品質承認ではない。maxQualityを知覚品質の百分率やlosslessと扱わず、今回A/Bの同一hashと上記表示寸法に限る。

## 負例と部分状態

[各1回のraw/exit/snapshot](evidence/negative/)を保存。負例は正常入力18/18へ加算しない。

| 負例 | 実exit・phase/code | 状態 |
|---|---|---|
| 画像でないtext | 1、preflight/E131 | 新siteとimages親は空、成功HTMLなし、画像parent内stagingなし |
| small WebP64 / 1B / max30 | 1、processing/E300 | 同上、causeにquality30–30でtargetBytes1不適合 |
| B1成功siteへ再実行 | 1、アプリmkdir（phase=null）/EEXIST | compile前拒否、既存29filesのbefore/after hash完全一致 |

cleanupErrorの添付はなかった。確認範囲はowned output親であり、OS全tmpを走査してprivate snapshot不存在まで証明していない。複数画像の途中失敗なら既に確定した画像は残り得る。この単一page consumerは全compile成功後にHTMLを書き、サイト全体のrollback/atomic更新を提供しない。

## 導入手順・つまずき・未確認

install、fixture/license照合と小画像生成、各画像policyの固定、公開compileImage呼出し、manifestの相対URL/srcset/sizes・HTML生成、静的配信が必要だった。アプリ本体はbuild.mjsの約60行（文章/CSS込み）、残りは測定・観測用。画像検証・staging transactionは再実装していない。

- directory作成前にworkdirを指定した最初のinstall tool呼出しはprocess未起動。作成後のinstall/prepareは実exit0を保存。
- CUA read-only DOMにはimage.decodeがなくTypeError。測定外previewのqa.jsがbrowser内decode/Resource Timingを可視DOMへ表示する方式に変更した。主6buildのコード/hashは不変。
- 18画像の大きな比較sheetで初回metadata A、fresh URLの2回目で派生原画像がdecode失敗（両方EncodingError、当該resourceStatus/bodyは0）。same-byte HTTP照合は成功したが、原因は未確定。[2回の失敗JSON/画面](evidence/browser/)を保持する。原図と候補の高さ端数差もあったため、測定外表示を実寸法へ揃えた個別3画像比較で、6点すべてを観察した。18画像同時比較の成功や原因解消は主張しない。
- quality collectorはloading文字列をJSON化して一部保存後に停止した。再開時の保存名誤指定はwxのEEXISTで拒否され、既存記録は不変。現在URL/保存済みfileを照合し、観測完了を待って残りを保存した。これらは評価補助の失敗でNative build失敗ではない。
- browser auditの初回assertはJPEG診断のplaceholder6件を数え落とし失敗。実12 resourceへ検査を直してexit0。全主build/negativeは再実行していない。

実利用者の採用、市場適合、全入力/全OS、DPR2実機、並列処理/cache最適化、Wasm/AVIF、一般SLA、native metrics内訳は今回未評価。公開1.5.0の厳しいbudgetでのE123制約はこの成功で解消していない。
