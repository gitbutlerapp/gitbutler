#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# An ad-hoc workspace with three dependent branches, each with one commit:
# main <- A <- B <- C, with C checked out.
git-init-frozen
commit-file init

git checkout main
  commit-file M
setup_target_to_match_main

git checkout -b A
  commit-file A
git checkout -b B
  commit-file B
git checkout -b C
  commit-file C
