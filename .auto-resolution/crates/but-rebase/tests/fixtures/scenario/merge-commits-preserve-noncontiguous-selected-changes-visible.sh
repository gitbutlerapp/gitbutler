#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# A visible merge-history variant of the noncontiguous-selection fixture.
# `HEAD` is a merge commit over:
# - branch `A` with commit `A`
# - branch `D` with commits `B -> C -> D`
# This lets the editor see both sides while verifying that selecting `A`, `B`,
# and `D` keeps `B` and `D` but excludes `C`.

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

tick
echo c >file-c
git add file-c
git commit -m C

tick
echo d >file-d
git add file-d
git commit -m D

git checkout main
tick
git merge --no-ff -m merge A B
