#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

# Local A holds `shared`; origin/A holds a rewritten copy of the same commit
# (different committer date, identical changes) plus one commit of its own.
git-init-frozen
commit-file M
setup_target_to_match_main

commit-file shared
git branch A
tick_committer
git commit --amend --no-edit
commit-file only-on-remote
git update-ref refs/remotes/origin/A HEAD
git reset --hard @~2

create_workspace_commit_once main A
