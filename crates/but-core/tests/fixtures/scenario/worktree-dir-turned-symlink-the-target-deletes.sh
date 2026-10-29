#!/usr/bin/env bash

set -eu -o pipefail

# `main` is checked out and `b` deletes `d/x`. The worktree no longer has the directory `d`,
# and the test puts a symlink to the ignored `elsewhere` in its place, as fixture copies don't keep
# symlinks.
git init -b main
mkdir d
printf 'one\n' >d/x
printf 'keep\n' >keep
git add d/x keep
git commit -m A

git switch -c b
git rm -q d/x
git commit -m B

git switch main
rm -r d
printf 'elsewhere\n' >>.git/info/exclude
mkdir elsewhere
printf 'elsewhere\n' >elsewhere/x
