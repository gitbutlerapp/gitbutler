#!/usr/bin/env bash

set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# The HEAD is detached, and there are multiple branches.

git init
commit M1
git branch B
git branch C
git checkout -b A
  commit A1
git checkout B
  commit B1
git checkout C
  commit C1
git checkout --detach HEAD
