#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
commit-file M
setup_target_to_match_main

git checkout -b A
commit-file A
create_workspace_commit_once A

git worktree add -b wt-on-A wt-on-A A
(cd wt-on-A
  commit-file W
)
