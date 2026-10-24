#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "two commits pushed to a remote, then rebased locally onto an advanced target.

The remote still has the old copy of both, plus a commit pushed from elsewhere.
A1 adds a file that A2 changes, so replaying A1 on top of the old copy conflicts." >.git/description

commit init
setup_target_to_match_main

git checkout -b A main
echo one >file && git add file && git commit -m A1
echo two >file && git commit -am A2

git checkout -b new-origin
echo remote >remote-file && git add remote-file && git commit -m R1
setup_remote_tracking new-origin A

git checkout main
echo target >target-file && git add target-file && git commit -m M1
remote_tracking_caught_up main

git checkout A
git rebase main

create_workspace_commit_once A
