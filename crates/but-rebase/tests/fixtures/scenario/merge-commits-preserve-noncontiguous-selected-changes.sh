#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# Two branches on top of a shared base:
# - `A` adds file-a
# - `B` adds file-b
# - `C` adds file-c on top of `B`
# - `D` adds file-d on top of `C`
#
# This is used to verify that selecting `A`, `B`, and `D` keeps the changes
# from `B` and `D` while excluding `C`.

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

tick
echo c >file-c
git add file-c
git commit -m C

tick
echo d >file-d
git add file-d
git commit -m D
