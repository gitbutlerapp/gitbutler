#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen
commit-file merge-base-message-that-is-intentionally-very-long-to-test-non-paged-truncation-in-status-output
setup_target_to_match_main

git checkout -b A
commit-file A
create_workspace_commit_once A

git checkout main
commit-file upstream-commit-message-that-is-intentionally-very-very-long-to-exceed-the-unpaged-width-limit-and-needs-truncation
git update-ref refs/remotes/origin/main HEAD

git checkout gitbutler/workspace
