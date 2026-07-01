#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit, with no stacks inside.
git-init-frozen
commit-file M
setup_target_to_match_main
create_workspace_commit_once
