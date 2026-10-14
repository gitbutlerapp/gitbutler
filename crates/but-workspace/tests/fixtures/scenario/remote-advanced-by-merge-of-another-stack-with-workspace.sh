#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

commit "init-integration"
setup_target_to_match_main

git checkout -b A main
commit "local change in A 1"
remote_tracking_caught_up A

git checkout -b C main
commit-file c1-file
remote_tracking_caught_up C

# On the remote, C gains a commit and is merged into A; local C stays behind.
git checkout -b new-origin-C C
commit-file c2-file
git checkout -b new-origin-A A
git merge --no-ff -m "Merge C into A" new-origin-C
git checkout A
setup_remote_tracking new-origin-A A 'move'
setup_remote_tracking new-origin-C C 'move'

create_workspace_commit_once A C
