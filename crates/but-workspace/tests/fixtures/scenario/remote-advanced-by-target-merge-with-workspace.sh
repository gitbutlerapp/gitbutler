#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

commit "init-integration"
setup_target_to_match_main

git checkout -b A main
commit "local change in A 1"
remote_tracking_caught_up A

git checkout main
commit-file target-file
remote_tracking_caught_up main

git checkout -b new-origin A
git merge --no-ff -m "Merge main into A" main
setup_remote_tracking new-origin A 'move'
git checkout A

create_workspace_commit_once A
