#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen
commit-file M
setup_target_to_match_main

git checkout -b A
commit-file only-on-remote
setup_remote_tracking A
git reset --hard HEAD~1
commit-file only-on-local
