#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit, with one stack inside with two branches.
# The bottom-most has a commit.
# The top-most is empty.
git init
commit M
setup_target_to_match_main

git checkout -b A
  commit A
git checkout -b B
create_workspace_commit_once B
