#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# A workspace commit with no applied stacks is based on C, while origin/main
# advances through a merge commit D and then X. Integrating upstream should move
# the workspace commit to X.
git init
commit-file C.txt C
setup_target_to_match_main
git branch gitbutler/target main

create_workspace_commit_once

git checkout -b A main
commit-file A.txt A

git checkout main
git merge --no-ff -m "D: merge A" A
commit-file X.txt X
git update-ref refs/remotes/origin/main main
git checkout gitbutler/workspace
