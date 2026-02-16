#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A workspace with two stacks and a dangling commit touching files from both branches
# This simulates a user making a commit that spans multiple virtual branches
git-init-frozen
commit-file M
setup_target_to_match_main

git branch B
git checkout -b A
  commit-file A
git checkout B
  commit-file B
create_workspace_commit_once A B

# Create a dangling commit on top of workspace touching both branches
git checkout gitbutler/workspace
  echo "modified" >> A
  echo "modified" >> B
  git add A B
  git commit -m "User commit touching both branches"
