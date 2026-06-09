#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Two branches are merged into a workspace commit. One branch is fully reachable
# from the target ref and should leave the workspace shape after integration,
# while the other branch remains in the workspace.
git init
commit M1
setup_target_to_match_main

git checkout -b A
commit-file A.txt A1

git checkout main
git checkout -b B
commit-file B.txt B1

create_workspace_commit_once A B

git update-ref refs/remotes/origin/main A
