#!/bin/bash

set -eu -o pipefail

git_root="$(git rev-parse --show-toplevel 2>/dev/null)"
cd "$git_root"

pnpm format

if ! git diff --quiet; then
  git diff --stat
  echo ""
  echo "Generated SDK types are out of date."
  echo "Run 'pnpm build:sdk && pnpm format' and commit the result."
  exit 2
fi
