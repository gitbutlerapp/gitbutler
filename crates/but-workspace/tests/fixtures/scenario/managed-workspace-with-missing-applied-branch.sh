#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# A managed workspace contains stacks A and B, but A's ref disappeared while its applied metadata
# remains. C is an independent branch ready to be applied.
git init
echo "A managed workspace with stale applied metadata for a branch whose ref disappeared" >.git/description

echo base >base
git add base
git commit -m M
setup_target_to_match_main

git branch B
git branch C
git checkout -b A
  commit-file A
git checkout B
  commit-file B
git checkout C
  commit-file C
git checkout B
create_workspace_commit_once A B

# An anonymous branch whose metadata will still be present.
git branch -D A
