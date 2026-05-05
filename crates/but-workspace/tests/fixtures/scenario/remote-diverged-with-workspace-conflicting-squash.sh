#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

tick
echo base >shared.txt
git add shared.txt
git commit -m "init-integration"
setup_target_to_match_main

git checkout -b A main
remote_tracking_caught_up A

tick
echo local >shared.txt
git add shared.txt
git commit -m "local change in A 1"

git checkout -b new-origin main

tick
echo remote >shared.txt
git add shared.txt
git commit -m "remote change in A 1"
setup_remote_tracking new-origin A

git checkout A

create_workspace_commit_once A
