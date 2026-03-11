#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen
commit-file M
setup_target_to_match_main

git checkout -b A
commit-file A
setup_remote_tracking A

echo amended > A
git add A
git commit --amend -m "add A amended"

create_workspace_commit_once A
