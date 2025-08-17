#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "Two commits in main, target setup, ws commit, many more usable branches" >.git/description

commit M1
commit M2
setup_target_to_match_main
git checkout -b A
commit A1
  git branch A1-1
  git branch A1-2
  git branch A1-3
commit A2
  git branch A2-1
  git branch A2-2
  git branch A2-3
create_workspace_commit_once A
