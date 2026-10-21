#!/usr/bin/env bash

set -eu -o pipefail

# `main` is checked out and `b` deletes `x`, which the worktree already deleted.
git init -b main
printf 'one\n' >x
printf 'keep\n' >keep
git add x keep
git commit -m A

git switch -c b
git rm -q x
git commit -m B

git switch main
rm x
