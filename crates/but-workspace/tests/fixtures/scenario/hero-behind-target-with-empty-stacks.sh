#!/usr/bin/env bash

set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# The workspace is an empty commit on an advanced target, with `lane-1` and `lane-2` parked
# on the base without content of their own. `hero` and `hero-clean` are based on the previous
# target position; `hero` changes the same file the target advanced with, `hero-clean` only
# adds an unrelated file.
git init
tick
echo original >shared.txt && echo original >shared2.txt && git add shared.txt shared2.txt && git commit -m M1
setup_target_to_match_main

git checkout -b hero main
tick
echo hero-change >shared.txt && echo hero-change >shared2.txt && git commit -am "hero: change shared files"

git checkout -b hero-clean main
tick
commit-file unrelated.txt

git checkout main
tick
echo target-advance >shared.txt && echo target-advance >shared2.txt && git commit -am "target: change shared files"
git update-ref refs/remotes/origin/main main

git branch lane-1 main
git branch lane-2 main

tick
create_workspace_commit_once
