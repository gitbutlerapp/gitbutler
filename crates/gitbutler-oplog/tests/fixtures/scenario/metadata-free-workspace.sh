#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/../../../../but/tests/fixtures/scenario/shared.sh"

git-init-frozen
git config user.name GitButler
git config user.email gitbutler@example.com
commit-file M
setup_target_to_match_main
create_workspace_commit_once
