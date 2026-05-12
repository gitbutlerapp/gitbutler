#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# A visible-history variant of the fail-fast-after-conflict fixture.
# `HEAD` includes branches `A`, `B`, and `C` in its ancestry, but the
# `A` and `B` changes still conflict when merged independently.

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

git checkout main
tick
git merge --no-ff -m merge-A A
tick
git merge --no-ff -X ours -m merge-B B
tick
git merge --no-ff -m merge-C C
