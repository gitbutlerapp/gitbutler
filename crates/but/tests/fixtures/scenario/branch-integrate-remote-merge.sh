#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen
commit-file M
setup_target_to_match_main

git checkout -b A
commit-file lane

git checkout -b feat
commit-file guard
git update-ref refs/remotes/origin/feat HEAD

git checkout -b remote-A A
git merge --no-ff -m "Merge feat into A" feat
git update-ref refs/remotes/origin/A HEAD

git checkout main
git branch -D feat remote-A
create_workspace_commit_once main A
