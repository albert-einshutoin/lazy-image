#!/bin/bash
# Complete setup and publish script

set -e

echo "📦 Platform Package Publisher - Setup"
echo "====================================="
echo ""

# Check GitHub CLI
if ! command -v gh &> /dev/null; then
  echo "❌ GitHub CLI not found. Installing..."
  brew install gh
fi

# Check authentication
if ! gh auth status &> /dev/null; then
  echo "⚠️  GitHub CLI not authenticated."
  echo ""
  echo "Please run: gh auth login"
  echo ""
  read -p "Press Enter after authentication is complete..."
fi

# Get latest workflow run
echo "🔍 Finding latest workflow run..."
RUN_ID=$(gh run list --limit 1 --json databaseId --jq '.[0].databaseId')

if [ -z "$RUN_ID" ]; then
  echo "❌ No workflow runs found"
  exit 1
fi

echo "✅ Found run: $RUN_ID"
echo ""

# Download artifacts
echo "📥 Downloading artifacts..."
mkdir -p artifacts
gh run download "$RUN_ID" --dir artifacts

if [ ! -d "artifacts" ] || [ -z "$(ls -A artifacts 2>/dev/null)" ]; then
  echo "❌ Failed to download artifacts"
  exit 1
fi

echo "✅ Artifacts downloaded"
echo ""

# Publish
echo "🚀 Publishing packages..."
./scripts/publish-all-platforms.sh
