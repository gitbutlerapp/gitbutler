#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Like `one-stack.sh`, but in a SHA-256 repository.
git-init-frozen --object-format=sha256
commit-file M
setup_target_to_match_main

git checkout -b A
  commit-file A
create_workspace_commit_once A
