#!/usr/bin/env bash
#
# Checks that the provided AppStream metadata file contains release information
# for the given version.
#
# This is meant to run in CI to prevent stale AppStream metadata.
#
# Requires software:
#   - xmlstarlet

set -euo pipefail

VERSION="$1"
METADATA_FILE="$2"

if [ -z "$VERSION" ] || [ ! -f "$METADATA_FILE" ]; then 
  echo "usage: check-appstream-metadata.sh <VERSION> <METADATA_FILE>"
  exit 1
fi

element=$(xmlstarlet sel -t -c "/component/releases/release[@version=\"$VERSION\"]" -n "$METADATA_FILE")

if [ -z "$element" ]; then
  echo "ERROR: No release for $VERSION in '$METADATA_FILE'"
  exit 1
fi

url=$(echo "$element" | xmlstarlet sel -t -v "/release/url" || echo '')
expected_url="https://github.com/gitbutlerapp/gitbutler/releases/tag/release%2F$VERSION"
if [ "$url" != "$expected_url" ]; then
  echo "ERROR: Expected release URL for $VERSION not found in '$METADATA_FILE'."
  echo ""
  echo "Expected URL: $expected_url"
  echo ""
  echo "    $element"
  exit 1
fi

echo "Release $VERSION in '$METADATA_FILE' looks OK"
echo ""
echo "    $element"
