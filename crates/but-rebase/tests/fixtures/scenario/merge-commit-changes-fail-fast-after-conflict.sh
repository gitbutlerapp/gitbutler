#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# Three branches on top of a shared base:
# - `A` changes `shared.txt` to "A"
# - `B` changes `shared.txt` to "B" and conflicts with `A`
# - `C` adds `file-c`
#
# This is used to verify that merging `A`, `B`, and `C` stops folding after
# the `A` vs `B` conflict instead of also applying `C`.

git init

tick
echo base >shared.txt
git add shared.txt
git commit -m base

git branch B
git branch C

git checkout -b A
tick
echo A >shared.txt
git add shared.txt
git commit -m A

git checkout B
tick
echo B >shared.txt
git add shared.txt
git commit -m B

git checkout C
tick
echo c >file-c
git add file-c
git commit -m C
