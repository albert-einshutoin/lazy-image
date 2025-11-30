#!/bin/bash
# Publish a platform-specific package to npm
# Usage: ./scripts/publish-platform.sh <platform> <node-file>
# Example: ./scripts/publish-platform.sh darwin-x64 lazy-image.darwin-x64.node

set -e

PLATFORM=$1
NODE_FILE=$2

if [ -z "$PLATFORM" ] || [ -z "$NODE_FILE" ]; then
  echo "Usage: $0 <platform> <node-file>"
  echo "Example: $0 darwin-x64 lazy-image.darwin-x64.node"
  exit 1
fi

if [ ! -f "$NODE_FILE" ]; then
  echo "Error: Node file not found: $NODE_FILE"
  exit 1
fi

VERSION=$(node -p "require('./package.json').version")
PKG_NAME="@alberteinshutoin/lazy-image-${PLATFORM}"

# Extract OS and CPU from platform
OS_PART=$(echo "$PLATFORM" | cut -d'-' -f1)
CPU_PART=$(echo "$PLATFORM" | cut -d'-' -f2)

# Create directory structure
mkdir -p "npm/$PLATFORM"

# Copy .node file
cp "$NODE_FILE" "npm/$PLATFORM/lazy-image.${PLATFORM}.node"

# Generate package.json
node -e "
const fs = require('fs');
const pkg = {
  name: '$PKG_NAME',
  version: '$VERSION',
  description: 'Next-generation image processing engine - smaller files than sharp, powered by Rust + mozjpeg',
  main: 'lazy-image.${PLATFORM}.node',
  os: ['$OS_PART'],
  cpu: ['$CPU_PART'],
  files: ['lazy-image.${PLATFORM}.node'],
  license: 'MIT',
  repository: {
    type: 'git',
    url: 'https://github.com/albert-einshutoin/lazy-image'
  },
  engines: {
    node: '>= 18'
  }
};
fs.writeFileSync('npm/$PLATFORM/package.json', JSON.stringify(pkg, null, 2));
"

echo "✅ Created package structure for $PLATFORM"
echo "📦 Package: $PKG_NAME@$VERSION"
echo ""
echo "To publish, run:"
echo "  cd npm/$PLATFORM && npm publish --access public"

