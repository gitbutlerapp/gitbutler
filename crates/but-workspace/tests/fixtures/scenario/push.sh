#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init --bare remote.git
git init
commit M
git remote add origin ./remote.git
git push --quiet -u origin main

git checkout -b bottom main
commit bottom
git checkout -b middle
commit middle
git checkout -b top
commit top

git checkout -b solo main
commit solo

create_workspace_commit_once top solo
