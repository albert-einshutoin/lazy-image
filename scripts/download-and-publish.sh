#!/bin/bash
# Helper script to guide through downloading artifacts and publishing

echo "📦 Platform Package Publisher"
echo "=============================="
echo ""
echo "このスクリプトは、GitHub Actionsからアーティファクトをダウンロードして"
echo "プラットフォーム別パッケージを公開する手順を案内します。"
echo ""
echo "手順:"
echo "1. ブラウザで以下にアクセス:"
echo "   https://github.com/albert-einshutoin/lazy-image/actions"
echo ""
echo "2. v0.7.6のタグ実行を探す（または最新の成功した実行）"
echo ""
echo "3. 各プラットフォームのビルドジョブからアーティファクトをダウンロード:"
echo "   - Build - x86_64-apple-darwin"
echo "   - Build - x86_64-pc-windows-msvc"
echo "   - Build - x86_64-unknown-linux-gnu"
echo "   - Build - x86_64-unknown-linux-musl"
echo ""
echo "4. ダウンロードしたzipファイルを解凍:"
echo "   mkdir -p artifacts"
echo "   unzip bindings-x86_64-apple-darwin.zip -d artifacts/"
echo "   unzip bindings-x86_64-pc-windows-msvc.zip -d artifacts/"
echo "   unzip bindings-x86_64-unknown-linux-gnu.zip -d artifacts/"
echo "   unzip bindings-x86_64-unknown-linux-musl.zip -d artifacts/"
echo ""
echo "5. このスクリプトを再実行:"
echo "   ./scripts/download-and-publish.sh"
echo ""
echo "または、アーティファクトが準備できたら:"
echo "   ./scripts/publish-all-platforms.sh"
echo ""

# Check if artifacts exist
if [ -d "artifacts" ] && [ "$(ls -A artifacts 2>/dev/null)" ]; then
  echo "✅ Artifacts directory found. Checking contents..."
  echo ""
  for dir in artifacts/*/; do
    if [ -d "$dir" ]; then
      node_file=$(find "$dir" -name "*.node" -type f | head -1)
      if [ -n "$node_file" ]; then
        echo "  ✅ $(basename "$dir"): Found .node file"
      else
        echo "  ⚠️  $(basename "$dir"): No .node file found"
      fi
    fi
  done
  echo ""
  read -p "アーティファクトが準備できましたか？ (y/n) " -n 1 -r
  echo
  if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "🚀 Publishing all platforms..."
    ./scripts/publish-all-platforms.sh
  else
    echo "アーティファクトを準備してから再実行してください。"
  fi
else
  echo "❌ Artifacts directory not found or empty."
  echo "上記の手順に従ってアーティファクトをダウンロードしてください。"
fi
