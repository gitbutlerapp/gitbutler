#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# Two stacks on different commits of the target's history. `main` and `older` change `file` in
# different ways, and the workspace only merges cleanly as `newer` undoes the change of `main`.
git init
echo one >file && git add file && git commit -m "base"
git checkout -b older
echo three >file && git commit -am "older"

git checkout main
echo two >file && git commit -am "main"
setup_target_to_match_main
git checkout -b newer
echo one >file && git commit -am "newer"

git checkout older
git checkout -b gitbutler/workspace
git merge --no-ff -m "GitButler Workspace Commit" newer
