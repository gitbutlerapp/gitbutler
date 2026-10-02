#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# Branch A moves submodule `sm` to C while the target replaces it with a regular
# file, so integrating the target conflicts A on `sm` (gitlink base and theirs, blob ours).

git init sm-source
(cd sm-source
  echo A >f && git add f && git commit -m A
  git checkout -b c && echo C >f && git commit -am C
  git checkout -q main
)

git-init-frozen
echo /sm-source >>.git/info/exclude
git -c protocol.file.allow=always submodule add ./sm-source sm
git commit -m "base"
git update-ref refs/heads/base HEAD

git checkout -b A
git -C sm checkout -q c
git add sm && git commit -m "A-change"

git checkout main
git rm -q --cached sm
mv sm .git/sm-worktree
echo file-on-main >sm
git add sm && git commit -m "main-change"
setup_target_to_match_main

rm sm
git checkout A
rmdir sm && mv .git/sm-worktree sm
create_workspace_commit_once A
