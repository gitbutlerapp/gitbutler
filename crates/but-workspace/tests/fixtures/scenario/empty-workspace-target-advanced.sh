#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# A workspace commit with no applied stacks is based on M1, while origin/main
# has advanced to X. Integrating upstream should move the workspace commit to X.
git init
commit M1
setup_target_to_match_main
git branch gitbutler/target main

create_workspace_commit_once

git checkout main
commit-file X.txt X
git update-ref refs/remotes/origin/main main
git checkout gitbutler/workspace
