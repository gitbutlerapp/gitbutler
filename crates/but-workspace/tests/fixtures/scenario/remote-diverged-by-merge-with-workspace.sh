#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

commit "init-integration"
setup_target_to_match_main

git checkout -b A main
commit "local change in A 1"
remote_tracking_caught_up A

git checkout -b feat A
commit-file feat-file
git checkout -b new-origin A
git merge --no-ff -m "Merge feat into A" feat
setup_remote_tracking new-origin A 'move'
setup_remote_tracking feat feat
git checkout A
git branch -D feat
commit-file local-file

create_workspace_commit_once A
