#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A workspace with one stack whose branch and its remote both added a commit since they last met.
git-init-frozen
commit-file M
setup_target_to_match_main

git checkout -b branch
commit-file shared
git checkout -b pushed-by-someone-else
commit-file remote
setup_remote_tracking pushed-by-someone-else branch
git checkout branch
git branch -D pushed-by-someone-else
commit-file local
create_workspace_commit_once branch
