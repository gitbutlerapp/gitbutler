#!/usr/bin/env bash

### Fixtures for fork-shaped projections — stacks whose declared shape is a DAG,
### and lanes that fork without any declaration saying so. Both are
### inexpressible in toml metadata, so tests declare them via
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

# Two SEPARATELY DECLARED stacks that converge above the integration base. The
# target lags behind the commit both branched from, so S1 is a shared tail and
# the two lanes are ONE multi-tip class — a graph fact no declaration states.
# S1 carries no ref, the way a workspace whose target moved on leaves it.
#
#   ws commit
#   |\
#   | * B1 (B)
#   * | A1 (A)
#   |/
#   * S1
#   * base (main, origin/main)
git init converging-lanes
(cd converging-lanes
  commit base
  setup_target_to_match_main
  git checkout -b shared main
    commit S1
  git checkout -b A shared
    commit A1
  git checkout -b B shared
    commit B1
  git branch -D shared
  create_workspace_commit_once A B
)

# The converging shape with an EMPTY branch: `E` shares lane A's tip, so one of
# the two names that segment and the other has no commits of its own. An empty
# segment resolves through the lane it belongs to — never through the sibling
# LANE that happens to follow it in the segment list.
git init converging-lanes-empty
(cd converging-lanes-empty
  commit base
  setup_target_to_match_main
  git checkout -b shared main
    commit S1
  git checkout -b A shared
    commit A1
  git checkout -b B shared
    commit B1
  git branch -D shared
  git branch E A
  create_workspace_commit_once A B
)
