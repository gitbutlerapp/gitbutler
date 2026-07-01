#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "A main branch with a remote that is advanced by one" >.git/description

commit M1
commit only-on-remote
setup_target_to_match_main
git reset --hard @~1

git checkout -b soon-tracking-of-feature
  commit without-local-tracking
git checkout main
turn_into_remote_branch soon-tracking-of-feature feature
