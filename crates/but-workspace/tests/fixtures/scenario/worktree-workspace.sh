#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "a managed workspace with two stacks, plus linked worktrees based on it" >.git/description

commit M0
commit M1
setup_target_to_match_main

git checkout -b A
  commit A1
  commit A2
git checkout -b B main
  commit B1

create_workspace_commit_once B A

# Branches off A1, a commit *inside* the workspace, and adds a commit of its own.
git worktree add -b wt-inside wt-inside A~1
(cd wt-inside
  commit W1
)

# Sits exactly on the tip of stack A, so it owns no commits at all.
git worktree add --detach wt-at A

# Based on the target commit, so it stands outside the workspace entirely.
git worktree add -b wt-outside wt-outside main
(cd wt-outside
  commit O1
)

# Stacked on wt-inside's branch: owns only its own commit, resting on W1.
git worktree add -b wt-stacked wt-stacked wt-inside
(cd wt-stacked
  commit S1
)

# Branches off *below* the target without sitting on the target commit itself,
# so only the base commit being reachable from the target reveals it is outside.
git worktree add -b wt-below wt-below main~1
(cd wt-below
  commit U1
)

# Unrelated history - the walk can never reach the workspace or the target.
git checkout --orphan disjoint
commit D1
git checkout gitbutler/workspace
git worktree add wt-disjoint disjoint
