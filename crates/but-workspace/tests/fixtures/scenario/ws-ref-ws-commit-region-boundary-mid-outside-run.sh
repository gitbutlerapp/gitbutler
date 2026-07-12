#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref over one empty lane (W at the base), while the target advanced onto a merge
# whose side leg runs through an applied branch's territory: A rests two commits up a
# side leg (c4 <- c1 <- base), C and D rest mid-leg on c1, and c1 is also a fork (X's
# leg). The target region's boundary at c1 lands MID-RUN of A's advanced-outside run.
git init
commit M

setup_target_to_match_main

git checkout -b W
create_workspace_commit_once W

tick
base=$(git rev-parse main)
tree=$(git rev-parse main^{tree})
c1=$(git commit-tree -p "$base" -m 'c1' "$tree")
git branch C "$c1"
git branch D "$c1"
tick
c3=$(git commit-tree -p "$c1" -m 'c3' "$tree")
git branch X "$c3"
tick
c4=$(git commit-tree -p "$c1" -m 'c4' "$tree")
git branch A "$c4"
tick
c5=$(git commit-tree -p "$base" -p "$c4" -m 'c5 merge' "$tree")
tick
adv=$(git commit-tree -p "$c5" -m 'advanced' "$tree")
git update-ref refs/heads/main "$c5"
git update-ref refs/remotes/origin/main "$adv"
