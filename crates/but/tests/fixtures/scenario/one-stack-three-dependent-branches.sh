#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit with one stack inside.
# The stack has three dependent branches, each with one commit:
# A -> B -> C.
git-init-frozen
commit-file M
setup_target_to_match_main

git checkout -b A
  commit-file A
git checkout -b B
  commit-file B
git checkout -b C
  commit-file C
create_workspace_commit_once C
