#!/usr/bin/env bash

set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# The workspace is an empty commit on an advanced target with two content-less lanes parked
# on it, while `hero` is based on the previous target position and changes the file the
# target advanced with.
git-init-frozen
tick
echo original >shared.txt && git add shared.txt && git commit -m M1
setup_target_to_match_main

git checkout -b hero main
tick
echo hero-change >shared.txt && git commit -am 'hero: change shared.txt'

git checkout main
tick
echo target-advance >shared.txt && git commit -am 'target: change shared.txt'
git update-ref refs/remotes/origin/main main

git branch lane-1 main
git branch lane-2 main

tick
create_workspace_commit_once
