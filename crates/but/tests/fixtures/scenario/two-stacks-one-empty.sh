#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit, with two stacks inside.
# One with one commit, one empty
git-init-frozen
commit-file M
setup_target_to_match_main

git branch B
git checkout -b A
  commit-file A
git checkout B
# Empty branch B first so the deterministic helper keeps it as a real ws-commit parent;
# checking out the non-empty branch first would merge the base as a no-op and drop it.
create_workspace_commit_once B A
