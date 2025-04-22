#!/usr/bin/env bash

### Description
# A single default branch with 10 commits in it, and multiple segments
set -eu -o pipefail

git init
for count in $(seq 10); do
  echo $count >file && git add . && git commit -m $count
done

git branch above-10 main
git branch nine :/9
git branch six :/6
git branch three :/3
git branch one $':/1\n'
