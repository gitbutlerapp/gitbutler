#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A repository with a self-remote target for local-only merging.
# This tests the `but merge` command merging a branch into gb-local

git-init-frozen
commit M

# Tests enter a managed workspace, then create branches and commits.
git remote add gb-local .
git update-ref refs/remotes/gb-local/main HEAD
git symbolic-ref refs/remotes/gb-local/HEAD refs/remotes/gb-local/main
git branch --set-upstream-to=gb-local/main main
