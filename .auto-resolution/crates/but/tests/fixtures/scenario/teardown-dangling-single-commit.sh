#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A workspace with one stack and a dangling commit on top of the workspace commit
# This simulates a user making a commit directly on gitbutler/workspace
git-init-frozen
commit-file M
setup_target_to_match_main

git checkout -b A
  commit-file A
create_workspace_commit_once A

# Create a dangling commit on top of workspace
git checkout gitbutler/workspace
  commit-file UserFile "User commit on workspace"
