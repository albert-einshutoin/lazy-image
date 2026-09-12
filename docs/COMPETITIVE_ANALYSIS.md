# 競合比較とドキュメント整理の判断

確認日: 2026-09-12。lazy-image は checkout `8d30265` / package.json 1.3.0 を確認。
競合は以下の公式ドキュメントを調査した。実機比較、料金試算、利用者調査、npm 公開確認は行っていない。
「公式に記載された機能」「ローカル実装」「本資料の判断」を分け、未確認の機能を非対応と断定しない。

## 結論

lazy-image の差別化は **公開前の画像一式の生成・検証・manifest・ローカル確定を一つの API/CLI にまとめること** に置く。
現在の実装に根拠はあるが、競合が実現できない独占機能、性能の全面的優位、市場での採用優位は未証明。
「小さい・安全・Rust・複数サイズ」だけでは訴求が弱い。利用者が省ける公開前の実装と、その契約を入口で示す。
採用した方針の正本は [PROJECT_PHILOSOPHY.md](./PROJECT_PHILOSOPHY.md)。

## 公式ドキュメントから分かる違い

| 製品 / 比較対象の形態 | ドキュメントの入口と整理 | 確認できた機能 | lazy-image への示唆 |
|---|---|---|---|
| [sharp](https://sharp.pixelplumbing.com/) / 組み込みライブラリ | Installation → Usage → Input/Output/操作別 API | 多形式、合成、Stream/Buffer/file、libvips による領域処理 | 最も直接的な代替。関数一覧より先に、compiler が肩代わりする仕事を示す |
| [imgproxy](https://docs.imgproxy.net/) / 自前運用 HTTP サーバー | Getting started → Installation/Configuration → Usage、Cache、Monitoring | URL でオンデマンド変換。署名 URL、入力サイズ・画像寸法制限。OSS/Pro 表で機能を区別 | HTTP 変換と運用は競合の主戦場。事前生成したファイルを配信したい利用者に絞る |
| [Imageflow](https://docs.imageflow.io/) / library・CLI・server | 実行形態を先に選び、querystring / JSON API に進む | [JSON API](https://docs.imageflow.io/json/introduction.html) の steps/graph、複数入出力、security 制限 | CLI、宣言的処理、複数出力自体は差別化にならない。公開セットの検証・確定契約を示す |
| [Cloudinary](https://cloudinary.com/documentation) / メディアプラットフォーム | 製品別に Get Started / Guides / References / SDKs / Release Notes | [画像最適化](https://cloudinary.com/documentation/image_optimization)の自動 quality / format。アップロード・資産管理も文書化 | 運用を委託したい利用者には強い。lazy-image の組み込み処理とは責任範囲が異なる |
| [Cloudflare Images](https://developers.cloudflare.com/images/) / マネージド画像サービス | 自前ストレージから変換・配信する経路と、Images に保存する経路を最初に分岐 | Edge 変換、ホスト画像の variants、保存・最適化・配信 | 「自分のストレージを使える」だけでは差別化できない。実行と成果物を自分で管理する点を明確にする |

比較は製品選択のためであり、同じ性能試験をした順位表ではない。imgproxy Pro の機能を OSS の機能として扱わない。
Cloudinary / Cloudflare の提供価値には配信・運用が含まれるため、エンコード時間や API 数だけで比較しない。

## 差別化の根拠と限界

| 訴求 | 評価 | 根拠・使い方 |
|---|---|---|
| policy → responsive artifacts + placeholder + manifest → 検証後の directory rename | 主軸にできる | [API](./API.md#transactional-public-upload-compilation)、[compiler](../lib/artifact-compiler.js)、[planner](../lib/artifact-policy.js)。同一 filesystem・信頼できる親ディレクトリが前提 |
| ファイル単位の byte budget と未達時の失敗 | 主軸を補強 | compiler policy の `budgets`。単独 target-byte API の `budgetMet` と混同しない。要求した予算を満たすことと知覚品質の保証は別 |
| メタデータ除去・リソース制限 | 必須の基礎品質 | [sharp も既定で metadata を除去](https://sharp.pixelplumbing.com/api-output/)。imgproxy / Imageflow にも制限がある。安全性の順位は付けない |
| JPEG が常に小さい | 主軸にしない | [過去の2ケース](./TRUE_BENCHMARKS.md)は旧版・同じ quality 数値での結果。[sharp にも mozjpeg オプション](https://sharp.pixelplumbing.com/api-output/#jpeg)がある。encoder 選択だけを優位性にしない |
| 省メモリ・Rust | 実行方式として説明 | V8 を通さない入力でも native memory は必要。sharp も領域処理を文書化。RSS・並列数・入力・FFI/codec 境界を含めた評価が必要 |
| Wasm / Edge | 補助経路 | [Wasm API](./WASM_PACKAGE_API.md) は狭い upload preflight 用。Node compiler の互換実装や Edge 配信サービスとは説明しない |

compiler は source snapshot の cleanup を公開前に完了し、その後 manifest と画像一式を確定する。
失敗した処理は公開せず、未公開 staging の cleanup 失敗は診断に残る。呼出側はその診断を確認し、
既存出力を消して再実行するような暗黙の上書きを避ける。今回、処理の実装は変更していない。

## 文書構成への示唆

入口は [README](../README.md) → [利用目的別の目次](./README.md) →
[導入ガイド](./ADOPTION_GUIDE.md) → [API](./API.md) とする。
内部仕様、測定手順、過去の記録は [開発者向け目次](./DEVELOPMENT.md)に分ける。
方針は Philosophy、優先順位は Roadmap、版ごとの変更は CHANGELOG を正本とし、
同じ優先順位・数値・リリース状態を複数ページで再掲しない。

古い実装計画、重複した版履歴、未実測の ROI 資料は削除し、必要なら Git 履歴から参照する。
測定結果は履歴として保持する。文書サイト用の依存や build は追加しない。

## 次の投資判断

1. **導入可能性**: CMS upload / static build の利用者が、policy を作り、結果を確認し、既存配信に接続できるか確認する。つまずきと必要なアプリ側実装を記録する。市場適合はまだ仮説。
2. **品質とコストの証拠**: 既存の [release evidence 手順](./BENCHMARK_OPERATIONS.md#release-compiler-and-constrained-quality-evidence-769)を使う。正常画像の受入、予算未達、alpha/ICC、時間・RSS を失敗込みで報告。codec 単体と compiler 全体を分ける。
3. **codec 改善**: 同一入力と品質条件で改善が確認できたものを優先する。sharp の設定も固定・記録し、default 比較を最適設定との比較に読み替えない。
4. **範囲拡大**: HTTP サーバー、CDN、DAM、汎用編集、Wasm 全 API 化は今回の主軸に必要な根拠が出るまで優先しない。

完了した研究と、これから必要な顧客検証・性能測定を混同しない。日付や版を付けずに競合の機能比較を再利用しない。
