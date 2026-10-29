#!/usr/bin/env bash

set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen
commit M
setup_remote_tracking main
add_main_remote_setup

git checkout -b A
commit A
git checkout -b B
commit B

# The remote stack has equivalent commits with different IDs, as after a rebase.
git checkout -b remote-A main
tick
commit A
git checkout -b remote-B
commit B
setup_remote_tracking remote-A A move
setup_remote_tracking remote-B B move

git checkout B
git branch --set-upstream-to=origin/A A
git branch --set-upstream-to=origin/B B
