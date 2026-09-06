#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref over one ANONYMOUS lane (its tip ref was deleted after the workspace commit),
# whose declared chain [A, D] has content only OUTSIDE the workspace: A rests on a side
# leg off the base, D rests empty on the lane's parent c3 — the target's local (main)
# position, with origin/main advanced one commit past it.
git init
commit M

setup_target_to_match_main

tick
tree=$(git rev-parse main^{tree})
base=$(git rev-parse main)
c3=$(git commit-tree -p "$base" -m 'c3' "$tree")
git branch D "$c3"
tick
c4=$(git commit-tree -p "$c3" -m 'c4' "$tree")
tick
c2=$(git commit-tree -p "$base" -m 'c2' "$tree")
git branch A "$c2"

git branch lane "$c4"
git checkout lane
create_workspace_commit_once lane
git branch -D lane

# main catches up to c3; origin/main advances one past it.
git update-ref refs/heads/main "$c3"
tick
adv=$(git commit-tree -p "$c3" -m 'advanced' "$tree")
git update-ref refs/remotes/origin/main "$adv"
