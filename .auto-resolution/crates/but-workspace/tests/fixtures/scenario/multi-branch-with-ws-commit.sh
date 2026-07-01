#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "two stacks, target, ws-commit, and ws-reference" >.git/description

commit M1
setup_target_to_match_main
git checkout -b A
git branch B
  commit A1
git checkout B
  commit B1

create_workspace_commit_once B A
