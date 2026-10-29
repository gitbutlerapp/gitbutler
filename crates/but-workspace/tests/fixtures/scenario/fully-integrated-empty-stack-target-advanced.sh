#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# An empty stack B points at old target C. The target ref advances to X.
git init
commit-file C.txt C
setup_target_to_match_main

git branch B

commit-file X.txt X
git update-ref refs/remotes/origin/main main

git checkout B
create_workspace_commit_once B
