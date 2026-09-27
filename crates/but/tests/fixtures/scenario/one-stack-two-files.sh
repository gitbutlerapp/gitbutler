#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# One stack with a single commit adding two files.
git-init-frozen
commit-file M
setup_target_to_match_main

git checkout -b A
  echo A >A
  echo B >B
  git add A B
  git commit -m "add A and B"
create_workspace_commit_once A
