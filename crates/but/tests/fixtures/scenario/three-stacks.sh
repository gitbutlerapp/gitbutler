#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit with three stacks inside.
# Stack A prepends lines to `file`.
# Stack B appends lines to `file` in two commits.
# Stack C creates `new-file` and extends it in two more commits.
git-init-frozen

seq 50 60 >file && git add . && git commit -m "base" && git tag base
setup_target_to_match_main

git branch B
git branch C

git checkout -b A
{ seq 10; seq 50 60; } >file && git add . && git commit -m "A: 10 lines on top"

git checkout B
{ seq 50 60; seq 61 70; } >file && git add . && git commit -m "B: 10 lines at the bottom"
{ seq 50 60; seq 61 80; } >file && git add . && git commit -m "B: another 10 lines at the bottom"

git checkout C
seq 10 >new-file && git add . && git commit -m "C: new file with 10 lines"
seq 20 >new-file && git add . && git commit -m "C: add 10 lines to new file"
seq 30 >new-file && git add . && git commit -m "C: add another 10 lines to new file"

create_workspace_commit_once A B C