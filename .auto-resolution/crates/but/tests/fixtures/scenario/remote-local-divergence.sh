#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen
commit-file M
setup_target_to_match_main
commit-file only-on-remote
git update-ref refs/remotes/origin/A HEAD
git reset --hard @~1

commit-file only-on-local
git branch A
git reset --hard @~1
create_workspace_commit_once main A

