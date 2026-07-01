#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A workspace with two stacks and two dangling commits on top
# First commit for branch A, second commit for branch B
git-init-frozen
commit-file M
setup_target_to_match_main

git branch B
git checkout -b A
  commit-file A
git checkout B
  commit-file B
create_workspace_commit_once A B

# Create two dangling commits on top of workspace
git checkout gitbutler/workspace
  commit-file FileForA "First user commit for A"
  commit-file FileForB "Second user commit for B"
