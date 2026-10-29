#!/bin/bash

set -eu -o pipefail

git init

git commit --allow-empty -m M

# Linked worktrees whose branches are one commit ahead of `main`, so their heads
# only appear in a graph that seeds worktree tips.
git worktree add -b feat-a wt-a
(cd wt-a
  git commit --allow-empty -m A1
)
git worktree add -b feat-b wt-b
(cd wt-b
  git commit --allow-empty -m B1
)
