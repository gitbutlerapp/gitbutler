#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A workspace with two dependent branches, each with one commit: A -> B.
git-init-frozen
commit-file M
setup_target_to_match_main

git checkout -b A
commit-file A
git checkout -b B
commit-file B
create_workspace_commit_once B
