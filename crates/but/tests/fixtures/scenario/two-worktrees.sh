#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit, with two stacks inside, each with their own commit.
git init
commit M1
commit M-base

git checkout -b A
  commit A

git checkout -b soon-origin-A main
  commit A-remote

git checkout -b B main
  commit B

git checkout main
  commit M-advanced
  setup_target_to_match_main

git checkout A
create_workspace_commit_once A B
setup_remote_tracking soon-origin-A A "move"

git worktree add .git/gitbutler/worktrees/A A
git worktree add .git/gitbutler/worktrees/B B

