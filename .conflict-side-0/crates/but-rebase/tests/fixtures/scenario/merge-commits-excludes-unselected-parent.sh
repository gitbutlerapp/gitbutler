#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# Two branches on top of a shared base:
# - `A` adds file-a
# - `B` adds file-b and `C` adds file-c on top of it
# This is used to verify that merging A and C does not implicitly pull in B.

git init

tick
echo base >base
git add base
git commit -m M

git branch B
git checkout -b A

tick
echo a >file-a
git add file-a
git commit -m A

git checkout B

tick
echo b >file-b
git add file-b
git commit -m B

git checkout -b C

tick
echo c >file-c
git add file-c
git commit -m C
