#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# A visible merge-history fixture where the first selected commit has its own
# parent chain that must be preserved.
# `HEAD` is a merge commit that contains both:
# - branch `D` with commits `A -> D`
# - branch `E` with commits `B -> C -> E`
# This lets the editor verify that selecting `D` and `E` keeps the first
# selected commit's full tree (`file-a`, `file-d`) while only applying `E`'s
# own delta (`file-e`) from the other branch.

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

git checkout -b D
tick
echo d >file-d
git add file-d
git commit -m D

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

git checkout -b E
tick
echo e >file-e
git add file-e
git commit -m E

git checkout main
tick
git merge --no-ff -m merge D E
