#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# A visible merge-history variant of the unselected-parent fixture.
# `HEAD` is a merge commit that contains both:
# - branch `A` with commit `A`
# - branch `C` with commits `B -> C`
# This lets the editor see both branches while verifying that selecting `A`
# and `C` does not pull in `B`.

git init

tick
echo base >base
git add base
git commit -m M

git checkout -b A
tick
echo a >file-a
git add file-a
git commit -m A

git checkout main
git checkout -b B
tick
echo b >file-b
git add file-b
git commit -m B

git checkout -b C
tick
echo c >file-c
git add file-c
git commit -m C

git checkout main
tick
git merge --no-ff -m merge A C
