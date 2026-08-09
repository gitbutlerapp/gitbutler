#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Two live heads share a two-commit bottom branch that was squash-merged upstream.
# The left head edits a bottom file; the right adds and reverts a file, leaving its
# final tree equal to bottom. Both heads must remain local and conflict-free.
git init
commit M1
git branch -M main
setup_target_to_match_main

git checkout -b bottom
commit-file bottom-1.txt bottom-1
commit-file bottom-2.txt bottom-2

git checkout -b left
commit-file bottom-2.txt left

git checkout bottom
git checkout -b right
commit-file temporary.txt temporary
git rm temporary.txt
git commit -m "revert temporary"

create_workspace_commit_once left right

git checkout -b upstream-main main
echo bottom-1 >bottom-1.txt
echo bottom-2 >bottom-2.txt
git add bottom-1.txt bottom-2.txt
git commit -m "squash bottom"
git update-ref refs/remotes/origin/main upstream-main

git checkout gitbutler/workspace
git branch -D upstream-main
