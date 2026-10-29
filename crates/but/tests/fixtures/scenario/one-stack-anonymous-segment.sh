#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A workspace with one named branch and an unreferenced commit above it, projected as an
# anonymous stack segment.
git-init-frozen
commit-file M
setup_target_to_match_main

git checkout -b A
  commit-file A

git checkout --detach
  commit anonymous
git branch -f A HEAD^

# HEAD remains on the anonymous commit, so the workspace commit is created above it.
create_workspace_commit_once
