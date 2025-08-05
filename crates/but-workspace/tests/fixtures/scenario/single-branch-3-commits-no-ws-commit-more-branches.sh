#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "Single commit, target, no ws commit, but ws-reference and a named segment, and branches on each commit" >.git/description

commit M1
setup_target_to_match_main
git checkout -b A
commit A1
  git branch A1
commit A2
  git branch A2
git checkout -b gitbutler/workspace
