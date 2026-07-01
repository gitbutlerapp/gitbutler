#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit, with two stacks inside:
# - One with a single branch with a single commit
# - Another with two branches, each with a single commit
git init
commit M
setup_target_to_match_main

git branch B
git checkout -b A
  commit A
git checkout B
  commit B
git checkout B -b C
  commit C
create_workspace_commit_once A C
