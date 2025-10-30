#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit, with one stack inside, with its own commit.
git init
commit-file M
setup_target_to_match_main

git checkout -b A
  commit-file A
create_workspace_commit_once A
