#!/usr/bin/env bash

### Description
source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "Single commit, target, no ws commit, but ws-reference" >.git/description

commit M1
setup_target_to_match_main
git checkout -b gitbutler/workspace
