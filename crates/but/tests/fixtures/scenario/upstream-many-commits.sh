#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen
commit-file M
setup_target_to_match_main

git checkout -b A
commit-file A
create_workspace_commit_once A

git checkout main
for i in $(seq 1 10); do
  commit-file "upstream-${i}"
done
git update-ref refs/remotes/origin/main HEAD

git checkout gitbutler/workspace
