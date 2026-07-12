#!/usr/bin/env bash

### Description
# Single-branch (ad-hoc) mode: a commit-owning branch with two empty branches
# stacked on its tip, over a base branch.
#   single-branch-fixture (base) <- commit-branch (owns 1 commit) <- empty-low <- empty-top
# HEAD on empty-top; empty-low and empty-top both point at commit-branch's tip.
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
commit-file base base
git branch -M single-branch-fixture

git checkout -b commit-branch
commit-file c commit

git branch empty-low
git checkout -b empty-top
