#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

commit-file base

git checkout -b bottom
commit-file bottom
setup_remote_tracking bottom

git checkout -b top

git checkout main
git merge --no-ff -m "merge bottom" bottom
setup_target_to_match_main

git checkout top
