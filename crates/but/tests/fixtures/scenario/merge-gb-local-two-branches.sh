#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A repository with no remote, gb-local setup, and two branches with commits
# This tests the `but merge` command merging a branch into gb-local

git-init-frozen
commit M

# We'll let the test itself run `but setup` to create gb-local
# and then create the branches and commits
