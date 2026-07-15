#!/bin/bash

set -eu -o pipefail

git init

echo "base" >base && git add . && git commit -m "base"
echo "a" >a && git add . && git commit -m "a"
git branch middle
echo "b" >b && git add . && git commit -m "b"

# A linked worktree named 'wt' checked out on 'middle' (inside workspace history).
git worktree add wt middle
