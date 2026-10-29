#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

commit "init-integration"
setup_target_to_match_main

git checkout -b A main
commit "local change in A 1"
remote_tracking_caught_up A

git checkout -b feat1 A
commit-file feat1-file
git checkout -b feat2 A
commit-file feat2-file
git checkout -b new-origin A
git merge --no-ff -m "Merge feat1 and feat2 into A" feat1 feat2
setup_remote_tracking new-origin A 'move'
git checkout A
git branch -D feat1 feat2

create_workspace_commit_once A
