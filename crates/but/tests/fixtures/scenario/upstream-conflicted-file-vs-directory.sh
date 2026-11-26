#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# Branch A turns file `d` into a directory, and the target changes `d`, so integrating the
# target conflicts A's commit on a file versus a directory.

git-init-frozen

echo base >d
git add d
git commit -m "base"
git update-ref refs/heads/base HEAD

git checkout -b A
git rm -q d
mkdir d
echo inside >d/f
git add d/f
git commit -m "A-change"

git checkout main
echo change-on-main >d
git add d
git commit -m "main-change"
setup_target_to_match_main

git checkout A
create_workspace_commit_once A
