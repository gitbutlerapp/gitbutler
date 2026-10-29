#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Two independent stacks are merged into a workspace commit. Both stacks are
# historically integrated into the target ref and should leave the workspace
# after integration.
git init
commit M1
setup_target_to_match_main

git checkout -b A
commit-file A.txt A1

git checkout main
git checkout -b B
commit-file B.txt B1

create_workspace_commit_once A B

git checkout main
git merge --no-ff -m "Merging A into base" A
git merge --no-ff -m "Merging B into base" B
git update-ref refs/remotes/origin/main main
git checkout gitbutler/workspace
