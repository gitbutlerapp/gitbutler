#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A branch has the short code `rr`, and its commit has a change id rzr.
git-init-frozen
commit-file M
setup_target_to_match_main

git checkout -b rr-branch
  echo branch >branch
  git add branch

  tree=$(git write-tree)
  parent=$(git rev-parse main)
  commit_id=$(git commit-tree "$tree" -p "$parent" -m "add branch 814")
  git update-ref refs/heads/rr-branch "$(add_change_id_to_given_commit rzr "$commit_id")"

create_workspace_commit_once rr-branch
