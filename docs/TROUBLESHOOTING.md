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

#### 3. 新しいトークンの作成

1. https://www.npmjs.com/settings/alberteinshutoin/tokens にアクセス
2. "Generate New Token" → "Automation" を選択
3. トークンをコピー
4. GitHubリポジトリの Settings → Secrets and variables → Actions
5. `NPM_TOKEN`シークレットを更新

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
- [ ] `@alberteinshutoin`スコープにアクセス権限がある
- [ ] スコープが組織の場合、メンバーである
- [ ] `package.json`に`publishConfig.access: "public"`が設定されている

### 関連ドキュメント

- [npm Publishing Scoped Packages](https://docs.npmjs.com/cli/v9/commands/npm-publish#publishing-scoped-packages)
- [GitHub Actions npm Authentication](https://docs.github.com/en/actions/publishing-packages/publishing-nodejs-packages)

