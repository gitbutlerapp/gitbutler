#!/usr/bin/env bash

### Description
set -eu -o pipefail

git init
echo "A single default branch with 10 commits in it." >.git/description
for count in $(seq 10); do
  echo $count >file && git add . && git commit -m $count
done
