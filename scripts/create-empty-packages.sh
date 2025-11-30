#!/bin/bash
# Create empty packages to "reserve" package names in npm
# Note: This is not recommended, but can be used to reserve package names
# The proper way is to publish packages with actual binaries

set -e

VERSION="0.0.0"  # Use 0.0.0 as placeholder version
PLATFORMS=(
  "darwin-x64"
  "win32-x64-msvc"
  "linux-x64-gnu"
  "linux-x64-musl"
)

echo "⚠️  Warning: Creating empty packages is not recommended."
echo "These packages will be created with version 0.0.0 as placeholders."
echo "You should publish actual packages with binaries as soon as possible."
echo ""
read -p "Continue? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
  echo "Cancelled."
  exit 0
fi

for platform in "${PLATFORMS[@]}"; do
  pkg_name="@alberteinshutoin/lazy-image-${platform}"
  
  echo ""
  echo "Creating $pkg_name..."
  
  # Create temporary directory
  temp_dir=$(mktemp -d)
  cd "$temp_dir"
  
  # Create minimal package.json
  cat > package.json <<PKGJSON
{
  "name": "$pkg_name",
  "version": "$VERSION",
  "description": "Placeholder package - will be replaced with actual binary",
  "main": "index.js",
  "files": ["index.js"],
  "license": "MIT",
  "publishConfig": {
    "access": "public"
  },
  "repository": {
    "type": "git",
    "url": "https://github.com/albert-einshutoin/lazy-image.git"
  }
}
PKGJSON
  
  # Create minimal index.js
  echo "module.exports = {};" > index.js
  
  # Publish
  if npm publish --access public; then
    echo "✅ Created $pkg_name@$VERSION"
  else
    echo "❌ Failed to create $pkg_name"
  fi
  
  # Cleanup
  cd - > /dev/null
  rm -rf "$temp_dir"
done

echo ""
echo "✅ All placeholder packages created."
echo "⚠️  Remember to publish actual packages with binaries as soon as possible!"
