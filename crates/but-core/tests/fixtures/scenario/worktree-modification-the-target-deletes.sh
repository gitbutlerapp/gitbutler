#!/usr/bin/env bash

set -eu -o pipefail

# `main` is checked out with `x` modified in the worktree, and `b` deletes `x`.
git init -b main
printf 'one\n' >x
printf 'keep\n' >keep
git add x keep
git commit -m A

git switch -c b
git rm -q x
git commit -m B

git switch main
printf 'two\n' >x
