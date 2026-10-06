#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# One branch with two commits, without a managed workspace reference.
# Pulling the target conflicts the bottom commit on shared.txt, while the top
# commit adds an independent file and stays clean. A remains checked out.
git-init-frozen

echo base >shared.txt
git add shared.txt
git commit -m "base"
setup_target_to_match_main
git remote set-url origin .

git checkout -b A
echo bottom >shared.txt
git add shared.txt
git commit -m "bottom change"
echo top >top.txt
git add top.txt
git commit -m "top change"

git checkout main
echo upstream >shared.txt
git add shared.txt
git commit -m "upstream change"
git update-ref refs/remotes/origin/main main

git checkout A
