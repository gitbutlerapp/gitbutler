#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "a shallow workspace plus a worktree on deep, ref-fragmented unrelated history" >.git/description

commit M1
setup_target_to_match_main

git checkout -b A
  commit A1

create_workspace_commit_once A

# Unrelated history whose chain is split into many segments by branches -
# more segments than the target is generations deep.
git checkout --orphan disjoint
commit D1
git branch d1
commit D2
git branch d2
commit D3
git branch d3
commit D4
git branch d4
commit D5
git checkout gitbutler/workspace
git worktree add wt-deep disjoint
