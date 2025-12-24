# Contributing to lazy-image

lazy-image へのコントリビューションに興味を持っていただきありがとうございます！
このドキュメントでは、プロジェクトへの参加方法について説明します。

---

## 目次

1. [開発環境のセットアップ](#開発環境のセットアップ)
2. [PRの出し方](#prの出し方)
3. [コーディング規約](#コーディング規約)
4. [Issue報告のガイドライン](#issue報告のガイドライン)
5. [Feature Acceptance Rules](#feature-acceptance-rules)

---

## 開発環境のセットアップ

### 必要なツール

- **Node.js**: >= 18
- **Rust**: stable (最新推奨)
- **Cargo**: Rustに付属
- **npm**: Node.jsに付属

### セットアップ手順

```bash
# 1. リポジトリをクローン
git clone https://github.com/albert-einshutoin/lazy-image.git
cd lazy-image

# 2. Node.js依存関係をインストール
npm install

# 3. ネイティブモジュールをビルド
npm run build

# 4. テストを実行して環境を確認
npm test
```

### テストコマンド

```bash
# 全てのテストを実行
npm test

# JavaScript テストのみ
npm run test:js

# Rust テストのみ
npm run test:rust

# ベンチマークテスト
npm run test:bench

# TypeScript型チェック
npm run test:types
```

---

## PRの出し方

### ブランチ戦略（GitHub Flow）

1. **develop ブランチ**: 開発用のベースブランチ
2. **feature/\*** ブランチ: 新機能開発用
3. **fix/\*** ブランチ: バグ修正用
4. **main ブランチ**: リリース用（直接pushしない）

### PR作成の流れ

```bash
# 1. developブランチから最新を取得
git checkout develop
git pull origin develop

# 2. 作業ブランチを作成（Issue番号を含める）
git checkout -b feature/123-add-new-feature

# 3. 変更を実装

# 4. テストを実行
npm test

# 5. コミット（Conventional Commitsを推奨）
git commit -m "feat: add new feature (#123)"

# 6. プッシュしてPRを作成
git push origin feature/123-add-new-feature
gh pr create --base develop --title "feat: add new feature" --body "Closes #123"
```

### PRのチェックリスト

PRを作成する前に以下を確認してください：

- [ ] テストが全てパスしている
- [ ] 新機能には対応するテストを追加した
- [ ] コーディング規約に従っている
- [ ] 必要に応じてドキュメントを更新した
- [ ] Feature Acceptance Rules を満たしている（新機能の場合）

---

## コーディング規約

### Rust

- **フォーマット**: `cargo fmt` を使用
- **リント**: `cargo clippy` で警告がないこと
- **エラー処理**: `thiserror` を使用した構造化エラー
- **ドキュメント**: 公開APIにはdocコメントを記述

```bash
# フォーマットチェック
cargo fmt --check

# リントチェック
cargo clippy -- -D warnings
```

### JavaScript/TypeScript

- **スタイル**: プロジェクトの既存コードに従う
- **型定義**: 新しいAPIには `index.d.ts` を更新
- **テスト**: `test/js/specs/` にテストファイルを追加

### コミットメッセージ

[Conventional Commits](https://www.conventionalcommits.org/) を推奨します：

- `feat:` 新機能
- `fix:` バグ修正
- `docs:` ドキュメントのみの変更
- `test:` テストの追加・修正
- `chore:` ビルドプロセスやツールの変更
- `refactor:` リファクタリング
- `perf:` パフォーマンス改善

例：
```
feat: add AVIF alpha channel support (#42)
fix: resolve memory leak in batch processing
docs: update README with new API examples
```

---

## Issue報告のガイドライン

### バグ報告

バグを報告する際は、以下の情報を含めてください：

1. **環境情報**
   - OS（macOS, Linux, Windows）とバージョン
   - Node.js バージョン
   - lazy-image バージョン

2. **再現手順**
   - 問題を再現するための最小限のコード
   - 使用した入力画像（可能であれば）

3. **期待される動作**
   - 何が起こるべきだったか

4. **実際の動作**
   - 何が起こったか（エラーメッセージを含む）

### 機能リクエスト

新機能を提案する際は：

1. **ユースケースを説明する**
   - APIではなく、解決したい問題を説明してください
   
2. **Feature Acceptance Rules を確認する**
   - 以下のルールを満たしているか確認してください

3. **Issue タイトルの形式**
   - 不明確な場合: `Proposal: <機能名> (use case only)`

---

## Feature Acceptance Rules

> 詳細は [docs/ROADMAP.md](docs/ROADMAP.md) を参照してください。

新機能は以下の**すべての条件**を満たす必要があります：

### 1. Web画像最適化を直接改善する

- ファイルサイズ、品質、メモリ、パイプラインパフォーマンスの改善に貢献すること

### 2. ミニマルなAPI哲学に適合する

- 複雑な抽象化やフィーチャークリープを避ける
- シンプルで予測可能なAPIを維持

### 3. 速度やメモリ使用量を損なわない

- パフォーマンスの低下を引き起こさない

### 4. Web配信における実際のユースケースがある

- CDN、アップロードパイプライン、ビルド時最適化など

### 5. sharp/jimp領域に踏み込まない

以下は**受け入れない**機能です：

- ❌ テキスト描画、キャンバスプリミティブ
- ❌ 重いフィルター（ぼかし、シャープ化、アーティスティックエフェクト）
- ❌ GIF/APNGアニメーションや動画処理
- ❌ リアルタイム<10msでの高並行性画像処理

**いずれかの条件を満たさない場合、その機能はリジェクトされます。**

---

## 質問がある場合

- 既存の [Issues](https://github.com/albert-einshutoin/lazy-image/issues) を確認してください
- 見つからない場合は新しいIssueを作成してください
- [ROADMAP](docs/ROADMAP.md) で今後の計画を確認できます

lazy-image は焦点を絞ることで成功しています。
それ以外の機能は別のライブラリに属します。

---

皆様のコントリビューションをお待ちしております！ 🎉

