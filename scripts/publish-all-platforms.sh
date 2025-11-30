#!/bin/bash
# Helper script to publish all platform packages
# This assumes you have downloaded artifacts from GitHub Actions
# and extracted them to the artifacts/ directory

set -e

VERSION=$(node -p "require('./package.json').version")
REQUIRED_PLATFORMS=(
  "darwin-x64"
  "win32-x64-msvc"
  "linux-x64-gnu"
  "linux-x64-musl"
)

echo "📦 Publishing platform-specific packages for v${VERSION}"
echo ""

# Check if artifacts directory exists
if [ ! -d "artifacts" ]; then
  echo "❌ Error: artifacts/ directory not found"
  echo ""
  echo "Please download artifacts from GitHub Actions:"
  echo "1. Go to https://github.com/albert-einshutoin/lazy-image/actions"
  echo "2. Open the latest workflow run"
  echo "3. Download artifacts from each build job"
  echo "4. Extract them to artifacts/ directory with structure:"
  echo "   artifacts/bindings-{target}/lazy-image.{target}.node"
  exit 1
fi

# Map Rust targets to npm platform names
declare -A PLATFORM_MAP=(
  ["x86_64-apple-darwin"]="darwin-x64"
  ["aarch64-apple-darwin"]="darwin-arm64"
  ["x86_64-pc-windows-msvc"]="win32-x64-msvc"
  ["x86_64-unknown-linux-gnu"]="linux-x64-gnu"
  ["x86_64-unknown-linux-musl"]="linux-x64-musl"
)

# Process each artifact
for artifact_dir in artifacts/*/; do
  if [ ! -d "$artifact_dir" ]; then
    continue
  fi
  
  artifact_name=$(basename "$artifact_dir")
  rust_target="${artifact_name#bindings-}"
  platform="${PLATFORM_MAP[$rust_target]}"
  
  if [ -z "$platform" ]; then
    echo "⚠️  Unknown platform: $rust_target"
    continue
  fi
  
  # Find .node file
  node_file=$(find "$artifact_dir" -name "*.node" -type f | head -1)
  
  if [ -z "$node_file" ]; then
    echo "❌ No .node file found for $platform"
    continue
  fi
  
  echo "📦 Processing $platform..."
  ./scripts/publish-platform.sh "$platform" "$node_file"
  
  # Publish
  echo "🚀 Publishing $platform..."
  cd "npm/$platform"
  npm publish --access public
  cd ../..
  
  echo "✅ Published @alberteinshutoin/lazy-image-${platform}@${VERSION}"
  echo ""
done

echo "🎉 All platform packages published!"

