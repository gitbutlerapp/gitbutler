#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "Two-segment stack in the workspace for upstream integration validation" >.git/description

commit M
setup_target_to_match_main

git checkout -b A
commit A

git checkout -b B
commit B

create_workspace_commit_once B
