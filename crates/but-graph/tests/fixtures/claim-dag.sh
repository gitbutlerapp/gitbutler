#!/usr/bin/env bash

### Fixtures for the fork claimer (claimer v2): stacks whose declared shape is a
### DAG, inexpressible in toml metadata — tests declare them via
### `InMemoryRefMetadata` and drive `Workspace::from_head` directly.

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# The minimal fork: one stack whose tip merges two declared legs resting on the
# same base.
#
#   ws commit
#   * top (merge)
#   |\
#   | * R1 (right)
#   * | L1 (left)
#   |/
#   * base (main, origin/main)
git init diamond
(cd diamond
  commit base
  git checkout -b left main
    commit L1
  git checkout -b right main
    commit R1
  git checkout -b top left
    git merge right --no-ff -m "M1"
  setup_target_to_match_main
  create_workspace_commit_once top
)

# The diamond with a declared EMPTY branch on one leg: `mid` rests on `left`'s
# tip and declares `left` as its parent — the empty segment must splice on its
# DECLARED edge, not absorb elsewhere.
git init fork-empty-leg
(cd fork-empty-leg
  commit base
  git checkout -b left main
    commit L1
  git checkout -b right main
    commit R1
  git checkout -b top left
    git merge right --no-ff -m "M1"
  git branch mid left
  setup_target_to_match_main
  create_workspace_commit_once top
)

# A fork whose second leg is ANONYMOUS: the merge interior commit R1 carries no
# ref and no declaration — leftover cone territory.
git init fork-anon-leg
(cd fork-anon-leg
  commit base
  git checkout -b left main
    commit L1
  git checkout --detach main
    commit R1
  anon_leg=$(git rev-parse HEAD)
  git checkout -b top left
    git merge --no-ff -m "M1" "$anon_leg"
  setup_target_to_match_main
  create_workspace_commit_once top
)
