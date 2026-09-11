#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

# Local A holds `shared` on top of M; origin/A holds the same `shared` change
# rebased onto its own commit `other`, which touches a different file.
git-init-frozen
commit-file M
setup_target_to_match_main

commit-file shared
git branch A
git reset --hard @~1

commit-file other
commit-file shared
git update-ref refs/remotes/origin/A HEAD
git reset --hard @~2

create_workspace_commit_once main A
