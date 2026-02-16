#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# 2 commits start with 5c.
git-init-frozen
commit-file M
setup_target_to_match_main

git checkout -b A
  commit-file A1
  commit-file A2
  commit-file A3
  commit-file A4
  commit-file A5
  commit-file A6
  commit-file A7
  commit-file A8
  commit-file A9
  commit-file A10
  commit-file A11
  commit-file A12
  commit-file A13
create_workspace_commit_once A
