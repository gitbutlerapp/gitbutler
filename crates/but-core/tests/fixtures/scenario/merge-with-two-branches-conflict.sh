#!/usr/bin/env bash

### Description
# Two branches on top of a common base, one commit each, with conflicting content.
# The merge is conflicting, the worktree is left in that state as well.
set -eu -o pipefail

git init
touch file
git add . && git commit -m init

git branch B
git checkout -b A
seq 10 20 >file && git commit -am "10 to 20"

git checkout B
seq 20 30 >file && git commit -am "20 to 30"

git checkout -b merge
git merge -m "merge A and B - conflicting" A || :
