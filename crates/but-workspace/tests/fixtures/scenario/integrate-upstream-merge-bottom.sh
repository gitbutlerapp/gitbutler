#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "Stack whose bottom commit is a merge commit for upstream integration" >.git/description

commit M1
setup_target_to_match_main

git checkout -b side
commit side

git checkout main
git checkout -b A
git merge --no-ff -m "merge-bottom" side

git checkout main
commit upstream
setup_remote_tracking main

git checkout -b gitbutler/workspace A
