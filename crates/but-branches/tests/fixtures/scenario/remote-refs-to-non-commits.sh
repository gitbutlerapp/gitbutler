#!/usr/bin/env bash

### Description
# Remote-tracking refs that point to a tree, to a blob, and to an annotated tag of
# a commit, next to an ordinary remote branch. A custom fetch refspec can map tags
# like these into `refs/remotes/`.
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

tick
commit-file "init"
setup_target_to_match_main

tick
git checkout -b feature main
commit "feature-1"
git tag -a -m "release" release
git checkout main
turn_into_remote_branch feature feature

git update-ref refs/remotes/origin/tree-snapshot 'main^{tree}'
git update-ref refs/remotes/origin/blob-snapshot main:init
git update-ref refs/remotes/origin/release "$(git rev-parse release)"
git tag -d release
