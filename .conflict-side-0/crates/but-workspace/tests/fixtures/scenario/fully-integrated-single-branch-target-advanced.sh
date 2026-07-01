#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A single stack points at A on top of the old target commit C, while the target ref has
# advanced to X. Integrating the fully integrated stack should leave the
# workspace commit parented to X.
git init
commit-file C.txt C
setup_target_to_match_main

git checkout -b A
commit-file B.txt B
commit-file A.txt A

git branch upstream-main

git checkout upstream-main
commit-file X.txt X
git update-ref refs/remotes/origin/main upstream-main

git checkout A
git branch -D upstream-main
create_workspace_commit_once A
