#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "Single-segment stack with an advanced target branch for upstream integration" >.git/description

commit M1
setup_target_to_match_main

git checkout -b A
commit A1
commit A2

git checkout main
commit upstream
setup_remote_tracking main

git checkout -b gitbutler/workspace A
