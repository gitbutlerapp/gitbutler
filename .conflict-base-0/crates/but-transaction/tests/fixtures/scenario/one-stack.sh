#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit, with one stack inside, with its own commit.
git-init-frozen
commit-file random-file
setup_target_to_match_main

git checkout -b branch
  commit-file file-one
  commit-file file-two
  commit-file file-three
create_workspace_commit_once branch
