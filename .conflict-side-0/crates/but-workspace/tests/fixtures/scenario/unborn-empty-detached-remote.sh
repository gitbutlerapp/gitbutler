#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# A newly initialized git repository, but with a known remote that has an object.

git init remote
(cd remote
  commit "M1"
)

git init unborn
(cd unborn
  git remote add orphan ../remote
  git fetch orphan
)
