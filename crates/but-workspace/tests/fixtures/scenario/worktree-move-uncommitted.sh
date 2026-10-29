#!/usr/bin/env bash

set -eu -o pipefail

git init

echo "base" >base && git add . && git commit -m "base"
printf "line1\nline2\nline3\nline4\nline5\nline6\nline7\n" >shared && git add shared && git commit -m "shared file"
echo "main-only" >main-file && git add main-file && git commit -m "main tip"

# A linked worktree on its own branch, forked below the main tip so the two
# checkouts' committed histories genuinely diverge (main-file only exists on
# main) without that divergence alone causing conflicts when moving.
git worktree add -b feat wt HEAD~1
(cd wt
  echo "wt-only" >wt-file
)
