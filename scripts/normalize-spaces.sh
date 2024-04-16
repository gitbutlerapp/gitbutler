#!/usr/bin/env bash
#
# Finds all files in a directory and renames all spaces to '+'.

set -euo pipefail

ROOT_DIR="${1:-}"

if [ -z "$ROOT_DIR" ]; then
  echo "Usage: $0 <directory>"
  exit 1
fi

find "$ROOT_DIR" -type f -name '*' -print0 | while IFS= read -r -d '' file; do
  mv -v "$file" "$(echo "$file" | tr ' ' '+')"
done
