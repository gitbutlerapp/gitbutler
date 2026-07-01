#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen

echo base >shared.txt
git add shared.txt
git commit -m "init-integration"
setup_target_to_match_main

git checkout -b A main
remote_tracking_caught_up A

echo local >shared.txt
git add shared.txt
git commit -m "local change in A"

git checkout -b new-origin main
echo remote >shared.txt
git add shared.txt
git commit -m "remote change in A"
setup_remote_tracking new-origin A

git checkout A
git branch -D new-origin
create_workspace_commit_once A
