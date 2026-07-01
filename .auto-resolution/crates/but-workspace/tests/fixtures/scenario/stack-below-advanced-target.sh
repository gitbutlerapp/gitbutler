#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A single applied stack 'A' based at M1, while the target branch (origin/main)
# is advanced by one commit (M2). The target tip therefore sits OUTSIDE the
# workspace. This mirrors the `but branch new` / create_virtual_branch repro:
# creating a no-anchor branch picks base = resolved_target_commit_id() == the
# origin/main tip (M2), which isn't part of the workspace.
git init
echo "A single applied stack below an advanced target branch" >.git/description

commit M1
git checkout -b A
  commit A1
git checkout main
  commit M2
  setup_target_to_match_main
git checkout A
create_workspace_commit_once A
