#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

# Local A ends in a merge that brings in `shared` from `side`; origin/A holds
# `shared` as a plain commit plus one commit of its own, so the replayed merge
# has nothing left to add.
git-init-frozen
commit-file M
setup_target_to_match_main

git checkout -b side
commit-file shared
git checkout main
git merge --no-ff -m "merge side" side
git branch A
git reset --hard @~1

tick_committer
commit-file shared
commit-file only-on-remote
git update-ref refs/remotes/origin/A HEAD
git reset --hard @~2

create_workspace_commit_once main A
