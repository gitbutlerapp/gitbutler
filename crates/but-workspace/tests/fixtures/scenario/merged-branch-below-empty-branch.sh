#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

commit-file base

git checkout -b bottom
commit-file bottom

git checkout -b top

mkdir -p .git/refs/remotes/origin
cp .git/refs/heads/bottom .git/refs/remotes/origin/bottom
cp .git/refs/heads/top .git/refs/remotes/origin/top
git config branch.bottom.remote origin
git config branch.bottom.merge refs/heads/bottom
git config branch.top.remote origin
git config branch.top.merge refs/heads/top

git checkout main
git merge --no-ff -m "merge bottom" bottom

setup_target_to_match_main

git checkout top
create_workspace_commit_once top
