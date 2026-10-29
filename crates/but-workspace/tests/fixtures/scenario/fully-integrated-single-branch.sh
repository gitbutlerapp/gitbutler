#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A workspace with only one branch, A. A is fully reachable from the target ref,
# so rebasing A should remove it from the workspace.
git init
commit M1
setup_target_to_match_main

git checkout -b A
commit-file A.txt A1

create_workspace_commit_once A

git update-ref refs/remotes/origin/main A
