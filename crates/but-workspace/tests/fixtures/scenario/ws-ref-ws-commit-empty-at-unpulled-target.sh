#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit over one stack (B with a commit), while the
# stack's declared chain also lists E: an empty branch resting exactly at the target's
# UNPULLED tip — main and origin/main both advanced one commit past the workspace's
# base, so the target's local shares E's commit.
git init
commit M

setup_target_to_match_main

git checkout -b B
  commit B1

create_workspace_commit_once B

# Advance main and origin/main one commit past M without updating the workspace.
tick
advanced=$(git commit-tree -p "$(git rev-parse main)" -m 'advanced' "$(git rev-parse main^{tree})")
git update-ref refs/heads/main "$advanced"
git update-ref refs/remotes/origin/main "$advanced"

# E rests exactly at the unpulled target tip.
git branch E "$advanced"
