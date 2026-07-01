#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A stack has commits B and A above old target C. The target ref has equivalent
# B/A changes with different commit IDs, plus X on top.
git init
commit-file C.txt C
setup_target_to_match_main

git checkout -b A
commit-file B.txt B
commit-file A.txt A

git checkout main
git cherry-pick A^
git cherry-pick A
commit-file X.txt X
git update-ref refs/remotes/origin/main main

git checkout A
create_workspace_commit_once A
