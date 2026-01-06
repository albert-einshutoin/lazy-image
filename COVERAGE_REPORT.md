# テストカバレッジ調査レポート

調査日: 2025年1月

## 概要

RustとJavaScriptの両方のテストカバレッジを測定しました。

## Rust カバレッジ結果

### 全体カバレッジ

| 指標 | カバレッジ | 詳細 |
|------|-----------|------|
| **Regions** | **78.95%** | 1007 regions中212 missed |
| **Functions** | **84.62%** | 247 functions中38 missed |
| **Lines** | **86.14%** | 2237 lines中310 missed |

### ファイル別カバレッジ

#### engine.rs
- **Regions**: 76.23% (816 regions中194 missed)
- **Functions**: 79.79% (188 functions中38 missed)
- **Lines**: 83.71% (1903 lines中310 missed)

#### error.rs
- **Regions**: 100.00% (30 regions中0 missed)
- **Functions**: 100.00% (22 functions中0 missed)
- **Lines**: 100.00% (137 lines中0 missed)

#### ops.rs
- **Regions**: 88.82% (161 regions中18 missed)
- **Functions**: 100.00% (37 functions中0 missed)
- **Lines**: 100.00% (197 lines中0 missed)

### テスト統計

- **ユニットテスト**: 128個（`src/`内）
- **統合テスト**: 35個（`tests/edge_cases.rs`）
- **合計**: 163個のテスト
- **すべてのテスト**: ✅ パス

### カバレッジが低い領域

`engine.rs`でカバレッジが低い主な理由：

1. **未使用コード**: 一部の関数やフィールドが未使用（警告あり）
   - `GLOBAL_THREAD_POOL`
   - `MAX_CONCURRENCY`
   - `MIN_RAYON_THREADS`
   - `validate_icc_profile()`
   - `extract_icc_profile()` など

2. **NAPI関連コード**: `--no-default-features`フラグでNAPI機能が無効化されているため、NAPI関連のコードがカバーされていない

3. **エッジケース**: 一部のエラーハンドリングパスがテストされていない可能性

## JavaScript カバレッジ結果

### 全体カバレッジ

| 指標 | カバレッジ |
|------|-----------|
| **Statements** | **21.72%** |
| **Branch** | **14.28%** |
| **Functions** | **50.00%** |
| **Lines** | **21.72%** |

### ファイル別カバレッジ

#### index.js
- **Statements**: 21.72%
- **Branch**: 14.28%
- **Functions**: 50.00%
- **Lines**: 21.72%

### テスト統計

- **統合テストファイル**: 7個
  - `basic.test.js`: 30テスト
  - `edge-cases.test.js`: 30テスト
  - `concurrency-validation.test.js`
  - `deprecation-warning.test.js`
  - `thread-pool-env.test.js`
  - `type-improvements.test.js`
  - `zero-copy-safety.test.js`
- **すべてのテスト**: ✅ パス

### カバレッジが低い理由

JavaScriptのカバレッジが低いのは**正常な状態**です：

1. **`index.js`は自動生成ファイル**: NAPI-RSによって自動生成されたバインディングファイル
2. **ロジックはRust側**: 実際の画像処理ロジックはすべてRust側に実装されている
3. **バインディングのみ**: JavaScript側はRust関数へのバインディングとエラーハンドリングのみ
4. **プラットフォーム分岐**: 多くのコードがプラットフォーム固有の分岐（未実行パスが多い）

## 推奨事項

### Rustカバレッジ改善

1. **未使用コードの削除**: 警告されている未使用の関数・フィールドを削除
2. **NAPI機能のテスト**: NAPI機能を有効化した状態でのテスト追加を検討
3. **エッジケースの追加**: カバーされていないエラーパスのテスト追加

### JavaScriptカバレッジ

- **現状維持で問題なし**: バインディングファイルのカバレッジは低くても問題ありません
- **統合テストで十分**: 実際の機能は統合テストで十分にカバーされています

## 測定コマンド

### Rust
```bash
cargo llvm-cov --no-default-features --summary-only
cargo llvm-cov --no-default-features --lcov --output-path rust-coverage.lcov
```

### JavaScript
```bash
npx c8 --reporter=text --reporter=html node test/integration/run.js
```

## 結論

- **Rust**: 約79-86%のカバレッジを達成。主要な機能は十分にテストされている
- **JavaScript**: バインディングファイルのため低カバレッジは正常。統合テストで機能は十分に検証されている

