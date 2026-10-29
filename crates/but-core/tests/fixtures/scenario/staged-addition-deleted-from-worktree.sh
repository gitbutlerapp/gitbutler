#!/usr/bin/env bash

set -eu -o pipefail

# `main` is checked out with `n` staged as new and then deleted from the worktree.
# `b` changes `keep` and doesn't know `n`.
git init -b main
printf 'keep\n' >keep
git add keep
git commit -m A

git switch -c b
printf 'changed\n' >keep
git commit -am B

git switch main
printf 'new\n' >n
git add n
rm n
