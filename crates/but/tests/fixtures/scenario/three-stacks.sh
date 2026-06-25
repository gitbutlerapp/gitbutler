#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit with three stacks inside.
# Stack A prepends lines to `file`.
# Stack B appends lines to `file` in two commits.
# Stack C creates `new-file` and extends it in two more commits.
# A and B branch from the target `base`; C branches one commit lower, so that
# emptying A and C (e.g. by moving their commits into B) leaves them on DISTINCT
# workspace-commit parents instead of collapsing onto a single shared base.
git-init-frozen

echo seed >seed && git add . && git commit -m "lower base"
seq 50 60 >file && git add . && git commit -m "base" && git tag base
setup_target_to_match_main

git branch B

git checkout -b A
{ seq 10; seq 50 60; } >file && git add . && git commit -m "A: 10 lines on top"

git checkout B
{ seq 50 60; seq 61 70; } >file && git add . && git commit -m "B: 10 lines at the bottom"
{ seq 50 60; seq 61 80; } >file && git add . && git commit -m "B: another 10 lines at the bottom"

git checkout -b C base~1
seq 10 >new-file && git add . && git commit -m "C: new file with 10 lines"
seq 20 >new-file && git add . && git commit -m "C: add 10 lines to new file"
seq 30 >new-file && git add . && git commit -m "C: add another 10 lines to new file"

create_workspace_commit_once A B C
