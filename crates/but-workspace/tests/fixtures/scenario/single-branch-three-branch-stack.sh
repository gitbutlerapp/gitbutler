#!/usr/bin/env bash

### Description
# A single-branch repository with three stacked non-empty branches:
# main <- A <- B <- C, with HEAD on C.
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
commit-file main main

git checkout -b A
commit-file a a

git checkout -b B
commit-file b b

git checkout -b C
commit-file c c
