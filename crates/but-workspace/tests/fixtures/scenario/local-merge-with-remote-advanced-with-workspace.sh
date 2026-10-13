#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

commit "init-integration"
setup_target_to_match_main

git checkout -b A main
commit "local change in A 1"
remote_tracking_caught_up A

git checkout -b new-origin A
commit-file remote-file
setup_remote_tracking new-origin A 'move'

git checkout -b side A
commit-file side-file
git checkout A
git merge --no-ff -m "Merge side into A" side
git branch -D side

create_workspace_commit_once A
