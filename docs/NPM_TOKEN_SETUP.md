# NPM_TOKEN セットアップガイド

## 問題

現在のトークンは`@alberteinshutoin/lazy-image`（メインパッケージ）へのアクセス権限しか持っていません。
プラットフォーム別パッケージ（`@alberteinshutoin/lazy-image-darwin-arm64`など）は別のパッケージとして扱われるため、
それらへのアクセス権限が必要です。

## 解決方法

### ステップ1: 新しいAutomationトークンの作成

1. https://www.npmjs.com/settings/alberteinshutoin/tokens にアクセス
2. "Generate New Token" をクリック
3. **"Automation"** タイプを選択
4. トークン名を入力（例: "GitHub Actions CI/CD"）

### ステップ2: スコープ全体へのアクセス権限を設定

**重要**: トークン作成時に、以下のいずれかを選択してください：

#### オプションA: スコープ全体へのアクセス（推奨）

- `@alberteinshutoin/*` へのアクセス権限を付与
- これにより、現在および将来のすべてのパッケージにアクセスできます

#### オプションB: 個別パッケージへのアクセス

以下の各パッケージを個別に追加：

- `@alberteinshutoin/lazy-image` (メインパッケージ)
- `@alberteinshutoin/lazy-image-darwin-arm64`
- `@alberteinshutoin/lazy-image-darwin-x64`
- `@alberteinshutoin/lazy-image-win32-x64-msvc`
- `@alberteinshutoin/lazy-image-linux-x64-gnu`
- `@alberteinshutoin/lazy-image-linux-x64-musl`

### ステップ3: トークンをGitHub Secretsに設定

1. 作成したトークンをコピー（一度しか表示されません）
2. GitHubリポジトリにアクセス: https://github.com/albert-einshutoin/lazy-image/settings/secrets/actions
3. `NPM_TOKEN`シークレットを編集（または新規作成）
4. トークンを貼り付けて保存

### ステップ4: 古いトークンの削除（オプション）

セキュリティのため、古いトークンは削除することを推奨します：

1. https://www.npmjs.com/settings/alberteinshutoin/tokens にアクセス
2. 古いトークンの "Delete token" をクリック

## 確認

トークンが正しく設定されたか確認するには：

1. GitHub Actionsで新しいワークフローを実行
2. "Verify npm authentication" ステップで `npm whoami` が成功することを確認
3. プラットフォームパッケージの公開が成功することを確認

## トラブルシューティング

### まだ404エラーが発生する場合

1. トークンがスコープ全体（`@alberteinshutoin/*`）へのアクセス権限を持っているか確認
2. GitHub Secretsの`NPM_TOKEN`が最新のトークンに更新されているか確認
3. トークンの有効期限が切れていないか確認

### ローカルでの確認

ローカルでトークンをテスト：

```bash
# 環境変数にトークンを設定
export NPM_TOKEN="your-token-here"

# npmに認証
echo "//registry.npmjs.org/:_authToken=$NPM_TOKEN" > ~/.npmrc

# 認証確認
npm whoami

# スコープへのアクセス確認
npm whoami --scope=@alberteinshutoin
```

