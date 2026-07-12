#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit, with two stacks inside, one empty and one with a commit.
git init
commit M
setup_target_to_match_main

git branch B
git checkout -b A
  commit A
git checkout B
# Reverse arg order on purpose: keep the empty branch B as parent[0] (diverging from
# metadata) for the *_display_order_follows_workspace_parents / empty-move tests.
create_workspace_commit_once B A
