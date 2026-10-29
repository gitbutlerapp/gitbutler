#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# A visible merge-history variant of the three-branch planning fixture.
# `main`, `left`, and `right` each have three commits on top of a shared base,
# and `HEAD` is an octopus merge of `left` and `right` into `main`.

git init

tick
echo base >base.txt
git add base.txt
git commit -m base

tick
echo main-1 >main.txt
git add main.txt
git commit -m main-1

tick
echo main-2 >main.txt
git add main.txt
git commit -m main-2

tick
echo main-3 >main.txt
git add main.txt
git commit -m main-3

git checkout -b left HEAD~3

tick
echo left-1 >left.txt
git add left.txt
git commit -m left-1

tick
echo left-2 >left.txt
git add left.txt
git commit -m left-2

tick
echo left-3 >left.txt
git add left.txt
git commit -m left-3

git checkout -b right main~3

tick
echo right-1 >right.txt
git add right.txt
git commit -m right-1

tick
echo right-2 >right.txt
git add right.txt
git commit -m right-2

tick
echo right-3 >right.txt
git add right.txt
git commit -m right-3

git checkout main
tick
git merge --no-ff --strategy octopus -m merged left right
