#!/usr/bin/env bash

### Description
# A workspace with one applied stack of two branches, plus every kind of branch
# the branch listing needs to classify: an unapplied two-branch stack (activated
# through metadata in the test), a standalone branch with a caught-up remote and
# an alias ref on the same commit, an unambiguous two-branch chain, a fork with
# shared private history, a remote-only branch, and a branch whose own remote
# lags behind its tip.
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

tick
commit "init"
setup_target_to_match_main

tick
git checkout -b applied-bottom main
commit "applied-bottom-1"
git checkout -b applied-top
commit "applied-top-1"

tick
git checkout -b unapplied-bottom main
commit "unapplied-bottom-1"
git checkout -b unapplied-top
commit "unapplied-top-1"

tick
git checkout -b standalone main
commit "standalone-1"
commit "standalone-2"
remote_tracking_caught_up standalone
git branch standalone-alias standalone

tick
git checkout -b chain-bottom main
commit "chain-bottom-1"
git checkout -b chain-top
commit "chain-top-1"

tick
git checkout -b fork-a main
commit "fork-shared"
git checkout -b fork-b
commit "fork-b-1"
git checkout fork-a
commit "fork-a-1"

tick
git checkout -b soon-remote-only main
commit "remote-only-1"
turn_into_remote_branch soon-remote-only remote-only

tick
git checkout -b remote-behind main
commit "remote-behind-1"
remote_tracking_caught_up remote-behind
commit "remote-behind-2"

tick
git checkout applied-top
create_workspace_commit_once applied-top
