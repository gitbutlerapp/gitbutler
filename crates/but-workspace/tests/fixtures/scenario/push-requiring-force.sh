#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init --bare remote.git
git init
commit M
git remote add origin ./remote.git
git push --quiet -u origin main

git checkout -b bottom main
commit bottom-v1
git push --quiet -u origin bottom
git rev-parse bottom >.git/refs/remotes/origin/bottom

git checkout -b remote-bottom bottom
commit remote-only-bottom
git push --quiet origin remote-bottom:bottom

git checkout bottom
git branch -D remote-bottom
git reset --hard main
commit bottom-v2

git checkout -b top
commit top

git checkout -b solo main
commit solo

create_workspace_commit_once top solo

