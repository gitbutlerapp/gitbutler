#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A stack has commits B and A above old target C. The target ref has the same
# B/A changes in a single squash commit, plus X on top.
git init
commit-file C.txt C
setup_target_to_match_main

git checkout -b A
commit-file B.txt B
commit-file A.txt A

git checkout main
git merge --squash A
git commit -m "Squash A into base"
commit-file X.txt X
git update-ref refs/remotes/origin/main main

git checkout A
create_workspace_commit_once A
