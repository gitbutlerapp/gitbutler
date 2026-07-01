#!/usr/bin/env bash
#
# Finds all files in a directory and renames all spaces to '_'.

set -euo pipefail

ROOT_DIR="${1:-}"

if [ -z "$ROOT_DIR" ]; then
    echo "Usage: $0 <directory>"
    exit 1
fi

find "$ROOT_DIR" -type f -name '*' -print0 | while IFS= read -r -d '' file; do
    new_file="$(echo "$file" | tr ' ' '_')"
    if [[ "$file" != "$new_file" ]]; then
        mv -v "$file" "$new_file"
    fi
done
