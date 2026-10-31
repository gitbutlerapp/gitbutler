#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/branch-listing.sh"

git config branch.main.remote origin
git config branch.main.merge refs/heads/main
git checkout main
tick
commit "local-target-1"
commit "local-target-2"
git checkout -b after-target
commit "after-target-1"
git checkout gitbutler/workspace
