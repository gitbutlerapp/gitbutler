#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

commit "init-integration"
setup_target_to_match_main

git checkout -b A main
commit "shared local/remote"
remote_tracking_caught_up A

# Local branch diverges from origin/A.
commit "local change in A 1"
commit "local change in A 2"

# Simulate a different remote tip for A.
git checkout -b new-origin A~2
commit "remote change in A 1"
commit "remote change in A 2"
setup_remote_tracking new-origin A 'move'

git checkout A

create_workspace_commit_once A