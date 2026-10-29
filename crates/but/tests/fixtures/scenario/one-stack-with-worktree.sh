#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit with a single stack, plus a linked worktree
# branched off that stack's commit with a commit and an uncommitted change of its own.
git-init-frozen
commit-file M
setup_target_to_match_main

git checkout -b A
  commit-file A
create_workspace_commit_once A

# A commit of its own keeps the worktree's branch ahead of the stack tip, so moving one is
# distinguishable from moving the other.
git worktree add -b wt-branch .git/gitbutler/test-worktrees/wt A
(cd .git/gitbutler/test-worktrees/wt
  commit-file W
  echo "worktree change" >wt-file.txt
)
