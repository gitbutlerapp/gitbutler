#!/usr/bin/env bash

### Description
# Two remote-only branches on the same commit of their own, as a teammate pushes
# one branch under two names, and a third a commit above them.
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

tick
commit "init"
setup_target_to_match_main

tick
git checkout -b soon-a main
commit "shared-1"
git branch soon-b
git checkout -b soon-above
commit "above-1"
git checkout main
turn_into_remote_branch soon-a team-a
turn_into_remote_branch soon-b team-b
turn_into_remote_branch soon-above above
