# Troubleshooting npm Publishing Issues

## 404 Error: Package Not Found in Registry

### 症状
```
npm error 404 Not Found - PUT https://registry.npmjs.org/@alberteinshutoin%2flazy-image-darwin-arm64
npm error 404  '@alberteinshutoin/lazy-image-darwin-arm64@0.7.7' is not in this registry.
```

### 原因

このエラーは、npmスコープ `@alberteinshutoin` への公開権限がないことを示しています。

### 解決方法

#### 1. NPM_TOKENの権限確認

GitHub Secretsの`NPM_TOKEN`が正しい権限を持っているか確認してください：

1. https://www.npmjs.com/settings/alberteinshutoin/tokens にアクセス
2. 使用しているトークンを確認
3. トークンに以下の権限があることを確認：
   - ✅ **Read and Write** (必須)
   - ✅ **Automation** (推奨、CI/CD用)

#### 2. スコープのアクセス権限確認

`@alberteinshutoin`が組織スコープの場合：

1. https://www.npmjs.com/org/alberteinshutoin にアクセス
2. あなたのアカウントがメンバーであることを確認
3. メンバーでない場合、組織のオーナーに追加を依頼

#### 3. 新しいトークンの作成（重要：スコープ全体へのアクセス）

現在のトークンは`@alberteinshutoin/lazy-image`（メインパッケージ）へのアクセス権限しか持っていません。
プラットフォーム別パッケージ（`@alberteinshutoin/lazy-image-darwin-arm64`など）は別のパッケージとして扱われるため、
**スコープ全体へのアクセス権限**が必要です。

**手順：**

1. 古いトークンを削除（必要に応じて）
2. https://www.npmjs.com/settings/alberteinshutoin/tokens にアクセス
3. "Generate New Token" → **"Automation"** を選択
4. **重要**: トークン作成時に、以下のいずれかを選択：
   - **オプションA（推奨）**: スコープ全体 `@alberteinshutoin/*` へのアクセス権限を付与
   - **オプションB**: 各プラットフォームパッケージを個別に追加
     - `@alberteinshutoin/lazy-image-darwin-arm64`
     - `@alberteinshutoin/lazy-image-darwin-x64`
     - `@alberteinshutoin/lazy-image-win32-x64-msvc`
     - `@alberteinshutoin/lazy-image-linux-x64-gnu`
     - `@alberteinshutoin/lazy-image-linux-x64-musl`
5. トークンをコピー（一度しか表示されません）
6. GitHubリポジトリの Settings → Secrets and variables → Actions
7. `NPM_TOKEN`シークレットを更新

#### 4. ローカルでの確認

ローカルで公開できるか確認：

```bash
# npmにログイン
npm login

# スコープへのアクセスを確認
npm whoami --scope=@alberteinshutoin

# 手動で公開を試みる
cd npm/darwin-arm64
npm publish --access public
```

#### 5. 一時的な解決策: ローカルから公開

CI/CDが動作しない場合、一時的にローカルから公開できます：

```bash
# 各プラットフォームのアーティファクトをダウンロード
# GitHub Actionsから手動でダウンロード、または:
gh run download <run-id> --dir artifacts

# 公開スクリプトを実行
./scripts/publish-all-platforms.sh
```

### 確認事項チェックリスト

- [ ] `NPM_TOKEN`がGitHub Secretsに設定されている
- [ ] トークンに「Read and Write」権限がある
- [ ] **重要**: トークンが`@alberteinshutoin/*`（スコープ全体）または各プラットフォームパッケージへのアクセス権限を持っている
- [ ] トークンが`@alberteinshutoin/lazy-image`（メインパッケージ）のみへのアクセス権限しか持っていない場合は、新しいトークンを作成
- [ ] `package.json`に`publishConfig.access: "public"`が設定されている

### 関連ドキュメント

- [npm Publishing Scoped Packages](https://docs.npmjs.com/cli/v9/commands/npm-publish#publishing-scoped-packages)
- [GitHub Actions npm Authentication](https://docs.github.com/en/actions/publishing-packages/publishing-nodejs-packages)

