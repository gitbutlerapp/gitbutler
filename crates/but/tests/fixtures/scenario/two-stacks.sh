#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit, with two stacks inside, each with their own commit.
git init
commit-file M
setup_target_to_match_main

git branch B
git checkout -b A
  commit-file A
git checkout B
  commit-file B
create_workspace_commit_once A B
