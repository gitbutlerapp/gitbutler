#!/usr/bin/env bash

set -eu -o pipefail

# `main` is checked out with the tracked directory `a` replaced by a file, and `b` deletes `a/b`.
git init -b main
mkdir a
printf 'one\n' >a/b
printf 'keep\n' >keep
git add a/b keep
git commit -m A

git switch -c b
git rm -q a/b
git commit -m B

git switch main
rm -r a
printf 'file\n' >a
