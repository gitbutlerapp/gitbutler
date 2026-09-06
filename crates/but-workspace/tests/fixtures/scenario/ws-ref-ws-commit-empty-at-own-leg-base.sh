#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref over one stack (E with a commit), whose declared chain [A, E] also lists A:
# an empty branch resting exactly on E's own leg base — the target tip main/origin/main
# share. The chain's content (E's run) is the carrier for A's splice.
git init
commit M
commit M2

setup_target_to_match_main
git branch A

git checkout -b E
  commit E1

create_workspace_commit_once E
