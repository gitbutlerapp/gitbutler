#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit over one stack (A with a commit), while a second
# declared chain has nothing above the workspace lower bound: B rests on an unreachable
# side leg, and D rests on the base commit BELOW the bound (origin/main advanced past it).
git init
commit M
git branch D

git checkout -b side
  commit S
git branch B
git checkout main
  commit M2

setup_target_to_match_main

# Advance origin/main past M2, making M2 the workspace lower bound and M integrated
# territory below it.
tick
git update-ref refs/remotes/origin/main "$(git commit-tree -p "$(git rev-parse refs/remotes/origin/main)" -m 'advanced' "$(git rev-parse main^{tree})")"

git checkout -b A
  commit A

create_workspace_commit_once A
