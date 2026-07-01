#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# A shared base with `left` and `right` branches. `left` has two commits, and
# `right` has one. `main` performs an octopus merge of `left` and `right`.

git init

tick
echo base >shared.txt
git add shared.txt
git commit -m base

git checkout -b left

tick
echo left-one >left.txt
git add left.txt
git commit -m left-1

tick
echo left-two >left.txt
git add left.txt
git commit -m left-2

git checkout main
git checkout -b right

tick
echo right >right.txt
git add right.txt
git commit -m right

git checkout main

tick
git merge --no-ff --strategy octopus -m octopus left right
