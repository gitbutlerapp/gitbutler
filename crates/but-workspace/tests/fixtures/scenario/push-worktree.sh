#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init --bare remote.git
git init
commit M
git remote add origin ./remote.git
git push --quiet -u origin main

git checkout -b bottom main
commit bottom
git checkout -b middle
commit middle
git checkout -b top
commit top

create_workspace_commit_once top

# A two-branch stack of its own, based on the target.
git worktree add -b wt-bottom wt-solo main
(cd wt-solo
  commit wt-bottom
  git checkout -b wt-top
  commit wt-top
)

# Rests on `middle`, a branch of the workspace stack.
git worktree add -b wt-on-middle wt-on-middle middle
(cd wt-on-middle
  commit wt-on-middle
)
