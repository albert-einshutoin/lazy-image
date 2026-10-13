# 今回の準備・観測補助の失敗

以下は今回のtool/command出力から記録した校正・準備結果。compilerの4正常呼出し/24成果物へ加算しない。旧#902のdecode失敗2件とも別。

| 段階 | 観測結果 | 影響と対応 |
|---|---|---|
| consumerコピー | exit1、既存package-lockへのCOPYFILE_EXCLがEEXIST | 既存ファイル不変をhash照合。未コピー3helperのみ追加。rendererの再実行なし |
| provenance guard | exit1、入力PNG hashと異なるSVGを渡したhelper拒否 | compiler前、imageStage null、出力なし。SVGをcompilerが処理した証拠ではない |
| browser tab screenshot | 撮影後に1440×1000/DPR2から1080×750/DPR1へ変わった | 当該shotをDPR2/可読性の証拠にしない。native App captureへ変更し前後実値を照合 |
| full-size PNG取得 | Chrome UIは688KB/完了、通常Downloads想定pathからのcopyはENOENT | 保存先/画素寸法未確認、採用証拠へ含めない。同じcaptureを再送して成功にしなかった |
| Downloads内部URL | browser security policyがchrome://downloads/を拒否 | native command/他surfaceで内部ページの取得を回避しなかった。許可されたlocal HTTPページのApp画面撮影を使用 |
| 緑ruler校正 | known-ruler検出check exit1 | App captureの実formatはJPEG。緑4pxは輸送圧縮で変わり、黒24pxを用いた75/100 CSS px校正へ変更。失敗recordは保存 |
| locator待機 | visible診断がある一方、waitFor selector期限切れ | waitFor依存を削除。既存の可視DOM JSONをparseし、実Image.decode結果を検証するcanonical読取へ変更。未準備ならJSON parseで失敗させる |
| touch emulation入力 | Input.dispatchMouseEvent期限切れ、Mobile1保存0件を確認 | Mobile(no touch)へ変更して同viewport/DPRで確認。実機tap成功とはしない |
| 初期拡大HTML | Mobileの実layout viewport1560×3376/DPR2 | 不合格HTML/JSONを保持。scroll領域で390×844を維持し、原寸960/2×1920、段階/左右操作を確認 |
| 拡大generic locator/native wheel | locator.press期限切れ、windowNotFoundAtPosition | 同じ物を成功と読み替えない。native/タブのTab/Space操作で実activeElement/scroll値を確認し、24 frameを保存 |

画面原本、JPEG format校正、入力拒否raw等はtask-local phaseにも保持。公開PNG cropは元の撮影JPEG損失を残す。等倍source-pixel画質の証拠は別のdecoded-file cropを使い、backend/DPRを推測で埋めない。
