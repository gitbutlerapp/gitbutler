#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/../../../../but/tests/fixtures/scenario/shared.sh"

git-init-frozen
git config user.name GitButler
git config user.email gitbutler@example.com
commit-file M
setup_target_to_match_main

git checkout -b A
commit-file first
create_workspace_commit_once A
git tag test-workspace-one

git checkout A
commit-file second
git checkout gitbutler/workspace
git merge --no-ff -m "GitButler Workspace Commit" A
git tag test-workspace-two
