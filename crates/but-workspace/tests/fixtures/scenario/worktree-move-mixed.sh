#!/bin/bash

set -eu -o pipefail

git init

echo "base" >base && git add . && git commit -m "base"
echo "stack base" >stack-file && git add . && git commit -m "stack base"
git branch stack-base
echo "workspace" >ws-file && git add . && git commit -m "workspace source"

git worktree add -b feat wt HEAD~2
(cd wt
  echo "worktree" >wt-file && git add . && git commit -m "worktree source"
)

git worktree add -b other other HEAD~2
git branch stable HEAD~2
git branch target HEAD~2
