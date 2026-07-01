#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "Two commits in main, target setup, ws commit" >.git/description

commit M1
commit M2
setup_target_to_match_main
git checkout -b A
commit A1
commit A2
create_workspace_commit_once A
