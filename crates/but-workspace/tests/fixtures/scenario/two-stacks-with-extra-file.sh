#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A workspace commit with two stacks.
# Stack A modifies `file` (prepends lines).
# Stack B creates `new-file`.
git init

seq 50 60 >file && git add . && git commit -m "base" && git tag base
setup_target_to_match_main

git branch B

git checkout -b A
{ seq 10; seq 50 60; } >file && git add . && git commit -m "A: 10 lines on top of file"

git checkout B
seq 10 >new-file && git add . && git commit -m "B: new file with 10 lines"

create_workspace_commit_once A B
