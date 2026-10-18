# #908 srcset初回選択と画像転送量

2026-10-18、Chrome **155.0.8059.85**、macOS27.0/arm64。Desktop ChromeのDevice Toolbar emulation、zoom100%、loopback HTTP/1.1。実機phone、可読性、codec品質、速度の試験ではない。[環境・順序](environment.json)、[全case集計](summary.json)、[原本対応](provenance.json)、[失敗](failures.json)を保存した。

## 初回24 load

各条件×responsive/固定1920×3 trial。各cold群の候補3 URLは新namespace内で未使用、pilotとは別集合。全24件で開始/終了/QAのviewportとDPRが一致し、CSS幅はDesktop960、Mobile342。全件でearly load、decode success、SW controller=null/registrations=[]、probeなし。固定対照は全て1920px、body185,508 B、Resource Timing transferSize185,808 Bだった。各cold pageに候補画像requestは1件で、追加・重複要求はなかった。

| 条件（viewport/DPR、CSS幅） | responsive選択幅 | 選択ファイル保存bytes＝server body B | RT transferSize B | body削減率 | RT transfer削減率 |
|---|---:|---:|---:|---:|---:|
| d1: 1440×1000 / 1、960 | 960 | 95,622 | 95,922 | 48.45% | 48.38% |
| d2: 1440×1000 / 2、960 | 1920 | 185,508 | 185,808 | 0% | 0% |
| m1: 390×844 / 1、342 | 684 | 64,168 | 64,468 | 65.41% | 65.30% |
| m2: 390×844 / 2、342 | 684 | 64,168 | 64,468 | 65.41% | 65.30% |

各行の3trialは同じ結果。削減率は同条件・同trialの `(固定−responsive)/固定`。trialを重み付けした実利用全体の削減率には変換していない。coldのencodedBodySize/decodedBodySizeは表のbody Bと一致した。naturalWidthは密度補正されるためencoded幅の根拠にはせず、固定manifest・URL・hashを使った。

## 取得済み対照6 load

Mobile DPR2で各trialの1920pxを固定ページでprimeした後、同じ候補URLのresponsiveへ移動した。3trial全てでcurrentSrcは1920px、decode successだったが、**684pxの追加ネットワーク取得も発生した**。

| trial | prime1920 request ID | warm HTML ID | warm684 request ID | warm選択1920 RT | warm画像body合計 B | warm RT transferSize合計 B |
|---|---:|---:|---:|---|---:|---:|
| 1 | 80 | 83 | 87 | cache / transferSize=0 | 64,168 | 64,468 |
| 2 | 94 | 97 | 101 | cache / transferSize=0 | 64,168 | 64,468 |
| 3 | 108 | 111 | 115 | cache / transferSize=0 | 64,168 | 64,468 |

primeはそれぞれ200、185,508 B、完了・非中断、`public, max-age=3600, immutable`。後続1920pxはdeliveryType=cache・transferSize=0で新規server requestなし、SW非介入、currentSrcとdecodeも対応するため再利用の根拠が揃う。0単独を根拠にしていない。追加684pxは200・完了64,168 B。warmのRT画像entryは各2件でencodedBodySize/decodedBodySize合計249,676 B（185,508＋64,168）。この合計はcached bodyを含み、ネットワークbody合計ではない。prime＋warmを一組にするとserver bodyは249,676 Bであり、primeの費用を消していない。追加取得が起きた内部原因はこの観測から断定しない。

## 実行・保存範囲

新namespace `0b91e082-429b-42d5-b671-c7c1d4278a37` の4pilot＋24cold＋3prime＋3warm＝34件は全件有効。serverの候補画像ネットワークrequestも34件で全件200・完了・非中断、Timingのcache entry4件はこの件数へ加えない。warm pilotも1920cache＋684networkだった。各pageはHTML requestから次document/idle requestまででserverを区切り、primeとwarmをtrialGroup単位で混ぜない。[summary.json](summary.json)に順序、raw位置、HTML hash、request ID、全candidate Timingを保持する。scriptやDevTools用404は画像転送量から除外したがserver原本には残した。

全3候補の保存容量は **345,298 B**（684:64,168、960:95,622、1920:185,508）。URL aliasが増えても保存画像を再生成していない。ファイルhashはcatalog、配信body hashはserver記録と照合した。serverのfinishはOSへ渡した範囲で、client受信bodyの直接hash証明ではない。RT transferSizeはbrowser API値で、packet captureや実HTTP headerの正確なbyte数ではない。

稼働browser full versionは同sessionのemulation前UADataを[initial/desktop-pilot.json](initial/desktop-pilot.json)に保存。custom profileではbrands/platform/fullVersionListが空なのでcaseごとはunknownのまま保持した。UA文字列のIntel/Mac10_15_7は実OS/CPUの根拠にしない。`chrome://version`を開く操作はURL policyに拒否され、回避していない。

最初の別namespaceのDesktop pilotは有効だったが正式集計へ含めない。Mobile pilotは開始1×844→終了4×3376、CSS0、natural寸法0、`unknown: empty-natural-dimensions`だった。viewport不正として棄却し、codec不具合や#904/#906の再現と解釈しない。UI操作のtimeout等11件、前面window競合、設定失敗も保存し、成功した再試行だけを履歴として残すことを避けた。完了後は所有4profileを削除し、emulation解除、所有tabとserverを閉じた。
