#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points at a workspace commit resting directly on the base M, while the
# single declared chain D → B → C → E → A owns nothing in the workspace: D rests on
# the target merge's second-parent leg, C and A exactly on the (unpulled) target
# tip — all true empties on integrated territory — and B and E rest on side legs
# outside the workspace cone. Builder-config fuzz seed 0.
git init
tree=$(git mktree </dev/null)
tick
c0=$(git commit-tree -m M "$tree")
tick
c1=$(git commit-tree -p "$c0" -m m1 "$tree")
tick
c2=$(git commit-tree -p "$c1" -m m2 "$tree")
tick
c3=$(git commit-tree -p "$c2" -m m3 "$tree")
tick
c4=$(git commit-tree -p "$c2" -m m4 "$tree")
tick
c6=$(git commit-tree -p "$c0" -m m6 "$tree")
tick
t=$(git commit-tree -p "$c1" -p "$c3" -m T "$tree")

git update-ref refs/heads/D "$c3"
git update-ref refs/heads/B "$c4"
git update-ref refs/heads/E "$c6"
git update-ref refs/heads/C "$t"
git update-ref refs/heads/A "$t"
git update-ref refs/heads/main "$t"
setup_target_to_match_main

tick
ws=$(git commit-tree -p "$c0" -m 'GitButler Workspace Commit' "$tree")
git update-ref refs/heads/gitbutler/workspace "$ws"
git symbolic-ref HEAD refs/heads/gitbutler/workspace
