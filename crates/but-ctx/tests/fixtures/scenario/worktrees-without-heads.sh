#!/bin/bash

set -eu -o pipefail

git init
git commit --allow-empty -m M

# Linked worktrees whose HEAD resolves to no commit, each for a different reason.

# The checkout was deleted without `git worktree remove`, leaving it prunable.
git worktree add -b feat-gone wt-gone
rm -rf wt-gone

# HEAD points at a branch that was never born.
git worktree add -b feat-unborn wt-unborn
git -C wt-unborn symbolic-ref HEAD refs/heads/never-born

# The workspace ref, which GitButler manages itself, is checked out.
git branch gitbutler/workspace
git worktree add wt-ws gitbutler/workspace
