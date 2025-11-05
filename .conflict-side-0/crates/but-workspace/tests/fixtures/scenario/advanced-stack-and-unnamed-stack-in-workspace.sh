#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "A workspace with an advanced stack whose ref exists, and another whose ref doesn't exist anymore" >.git/description

commit M1
setup_target_to_match_main
git checkout -b outside
  commit advanced-inside
git checkout -b soon-missing main
  commit missing-name
create_workspace_commit_once outside soon-missing
git checkout -b feature main
  commit F1

git checkout outside
  commit advanced-outside
git branch -D soon-missing
git checkout gitbutler/workspace
