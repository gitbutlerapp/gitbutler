#!/bin/bash

set -eu -o pipefail

git init

echo "base" >base && git add . && git commit -m "base"
echo "m1" >m1 && git add . && git commit -m "M1"

# A linked worktree 'wt' on 'feat', forked below the main tip, with one commit
# and two uncommitted changes: a tracked modification and an untracked addition.
git worktree add -b feat wt HEAD~1
(cd wt
  printf "one\ntwo\nthree\n" >a-file && git add a-file && git commit -m "F1"
  printf "one\ntwo\nthree\nfour\n" >a-file
  echo "new" >new-file
)
