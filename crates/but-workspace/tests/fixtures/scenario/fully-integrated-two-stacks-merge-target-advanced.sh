#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Two independent stacks A and B are merged into the target ref, which then
# advances to X. Integrating both should leave the workspace commit on X.
git init
commit-file C.txt C
setup_target_to_match_main

git checkout -b A
commit-file A.txt A

git checkout main
git checkout -b B
commit-file B.txt B

create_workspace_commit_once A B

git checkout main
git merge --no-ff -m "D: merge A" A
git merge --no-ff -m "E: merge B" B
commit-file X.txt X
git update-ref refs/remotes/origin/main main
git checkout gitbutler/workspace
