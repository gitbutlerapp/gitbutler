#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a workspace commit, with one stack inside with two branches their own commit.
git init
commit M
setup_target_to_match_main

git checkout -b A
  commit A
git checkout -b B
  commit B
create_workspace_commit_once B
