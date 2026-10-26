#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit merging the stacks A (two commits) and B (one commit).
# Around them are branches that aren't applied:
# - C: one commit of its own on the target.
# - conflicting: one commit on the target changing the file A changes.
git init
commit-file M
setup_target_to_match_main

git branch B
git branch C
git branch conflicting
git checkout -b A
  commit-file shared A1
  commit-file a A2
git checkout B
  commit-file b
git checkout C
  commit-file c
git checkout conflicting
  commit-file shared conflicting
git checkout A
create_workspace_commit_once A B
