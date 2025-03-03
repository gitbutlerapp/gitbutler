#!/usr/bin/env bash

### Description
# A single default branch with 10 commits in it.
set -eu -o pipefail

git init
for count in $(seq 10); do
  echo $count >file && git add . && git commit -m $count
done
