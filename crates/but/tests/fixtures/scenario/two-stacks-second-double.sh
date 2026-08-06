#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Two single-commit stacks anchored on DIFFERENT bases: A on the merge base M, B one
# commit back on M's parent Z. The moving_multiple test moves both commits out, emptying
# both stacks onto distinct bases — so they map to distinct workspace-commit parents and
# exercise parent-index stack ordering (not the same-base fallback).
git-init-frozen
commit-file Z
commit-file M
setup_target_to_match_main

git checkout -b A
  commit-file A
git checkout -b B main~1
  commit-file B
create_workspace_commit_once A B
