#!/bin/bash

# Vercel ignore build step script
# Configured here: https://vercel.com/gitbutler/gitbutler-web/settings/git
# Script checks if there are changes in specific directories
# Exit code 0 = skip build, Exit code 1 = proceed

echo "🔍 Checking for changes in monitored directories..."

# Directories to monitor for changes
MONITORED_DIRS=(
  "apps/web"
  "packages/shared" 
  "packages/core"
)

# Check if we're in a git repository
if ! git rev-parse --git-dir > /dev/null 2>&1; then
  echo "❌ Not in a git repository. Proceeding with build."
  exit 1
fi

# Get the previous commit SHA
if [ -n "$VERCEL_GIT_PREVIOUS_SHA" ]; then
  PREVIOUS_COMMIT="$VERCEL_GIT_PREVIOUS_SHA"
  echo "📊 Using Vercel previous deployment SHA: $PREVIOUS_COMMIT"
elif git rev-parse --verify HEAD^ > /dev/null 2>&1; then
  PREVIOUS_COMMIT="HEAD^"
  echo "📊 Using previous commit (HEAD^) as fallback"
else
  echo "⚠️  No previous commit reference found. Proceeding with build."
  exit 1
fi

# Verify the previous commit exists in the repository
if ! git rev-parse --verify "$PREVIOUS_COMMIT" > /dev/null 2>&1; then
  echo "⚠️  Previous commit $PREVIOUS_COMMIT not found in repository. Proceeding with build."
  exit 1
fi

echo "📊 Comparing $PREVIOUS_COMMIT with HEAD"

# Check each monitored directory for changes
HAS_CHANGES=false

for dir in "${MONITORED_DIRS[@]}"; do
  if [ -d "$dir" ]; then
    # Check if there are any changes in this directory
    if ! git diff --quiet "$PREVIOUS_COMMIT" HEAD -- "$dir"; then
      echo "✅ Changes detected in: $dir"
      HAS_CHANGES=true
    else
      echo "➖ No changes in: $dir"
    fi
  else
    echo "⚠️  Directory not found: $dir"
  fi
done

# Also check for changes to package.json files that might affect dependencies
ROOT_FILES=(
  "package.json"
  "package-lock.json"
  "yarn.lock"
  "pnpm-lock.yaml"
)

for file in "${ROOT_FILES[@]}"; do
  if [ -f "$file" ]; then
    if ! git diff --quiet "$PREVIOUS_COMMIT" HEAD -- "$file"; then
      echo "✅ Changes detected in root: $file"
      HAS_CHANGES=true
    fi
  fi
done

echo ""

if [ "$HAS_CHANGES" = true ]; then
  echo "🚀 Changes detected in monitored paths. Proceeding with build."
  exit 1  # Build
else
  echo "🛑 No relevant changes detected. Skipping build."
  exit 0  # Skip build
fi

