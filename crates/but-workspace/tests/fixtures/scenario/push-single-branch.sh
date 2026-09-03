#!/usr/bin/env bash

### Description
# Like `push.sh`, but with an ordinary branch checked out and no workspace commit:
# main <- bottom <- top, with HEAD on top.
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init --bare remote.git
git init
commit M
git remote add origin ./remote.git
git push --quiet -u origin main

git checkout -b bottom main
commit bottom
git checkout -b top
commit top
