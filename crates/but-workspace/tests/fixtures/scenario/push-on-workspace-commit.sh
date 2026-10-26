#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init --bare remote.git
git init
commit M
git remote add origin ./remote.git
git push --quiet -u origin main

git checkout -b A main
commit A

create_workspace_commit_once A

# Made from the main checkout's `HEAD`, as `git worktree add` there makes one, so it rests on
# the workspace commit.
git worktree add -b wt-on-workspace wt-on-workspace HEAD
(cd wt-on-workspace
  commit wt-on-workspace
)

# Rests on the worktree above, so the workspace commit is further down.
git worktree add -b wt-on-it wt-on-it wt-on-workspace
(cd wt-on-it
  commit wt-on-it
)
