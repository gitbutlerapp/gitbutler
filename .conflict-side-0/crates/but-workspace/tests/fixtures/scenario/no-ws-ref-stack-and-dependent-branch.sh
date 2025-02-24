#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A target branch along with a stack and a dependent branch at the same tip, without workspace commit.
git init
commit M
setup_target_to_match_main
git checkout -b A
commit A1
git branch D
git branch E
commit A2
git branch B
git branch C

git checkout main
