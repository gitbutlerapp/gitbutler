#!/usr/bin/env bash

### Description
# A workspace with one applied stack of two branches that was left with
# `git checkout main`, so HEAD is the target branch while the workspace
# metadata still has the stack applied.
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

tick
commit "init"
setup_target_to_match_main

tick
git checkout -b applied-bottom main
commit "applied-bottom-1"
git checkout -b applied-top
commit "applied-top-1"

create_workspace_commit_once applied-top
git checkout main
