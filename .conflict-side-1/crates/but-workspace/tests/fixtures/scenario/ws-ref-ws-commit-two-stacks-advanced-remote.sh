#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit, with two stacks inside:
# - Each stack has a single branch with a single commit
# - origin/main has advanced past the fork point where both branches diverge
# This means the base segment at the old fork point has no ref name.
git init
commit M
git branch A
git checkout -b B
  commit B
git checkout A
  commit A

# Set up the remote tracking to point at main *before* advancing it.
setup_target_to_match_main

# Advance origin/main past the fork point.
# This makes the original M commit an unnamed segment in the graph.
tick
git update-ref refs/remotes/origin/main "$(git commit-tree -p $(git rev-parse refs/remotes/origin/main) -m 'M2' $(git rev-parse main^{tree}))"

create_workspace_commit_once A B
