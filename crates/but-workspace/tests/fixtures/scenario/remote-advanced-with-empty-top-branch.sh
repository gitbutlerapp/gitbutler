#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A workspace has a local branch whose upstream has advanced, and a parallel
# empty branch. This mirrors `but apply feature-foo` followed by
# `but branch new empty-branch`.
git init
echo "A remote-advanced branch with a parallel empty branch" >.git/description

commit "add main.txt"
setup_target_to_match_main

git checkout -b feature-foo main
commit "add foo.txt"
remote_tracking_caught_up feature-foo

git checkout -b new-origin-feature-foo feature-foo
commit "update foo.txt (remote)"
setup_remote_tracking new-origin-feature-foo feature-foo 'move'

git checkout main
git branch empty-branch
git checkout feature-foo
create_workspace_commit_once feature-foo
