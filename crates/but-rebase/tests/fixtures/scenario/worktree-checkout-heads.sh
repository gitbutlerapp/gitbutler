#!/bin/bash

set -eu -o pipefail

git init

echo "base" >base && git add . && git commit -m "base"
echo "a" >a && git add . && git commit -m "a"
git branch middle
echo "b" >b && git add . && git commit -m "b"

git worktree add wt middle
git worktree add --detach wt-detached middle
