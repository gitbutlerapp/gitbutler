#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# Working directly on main, which tracks the target origin/main. The target has
# moved on by one commit since, and main hasn't.
git init
commit-file file.txt one
setup_target_to_match_main
git config branch.main.remote origin
git config branch.main.merge refs/heads/main

git checkout -b advanced-target
commit-file file.txt two
git update-ref refs/remotes/origin/main advanced-target
git checkout main
git branch -D advanced-target
