#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# Branch A and the target both move submodule `sm` (to C and B, both children of
# its base commit A) and change `README`, so integrating the target conflicts A on both.
# `sm` stays checked out at A's side, C.

git init sm-source
(cd sm-source
  echo A >f && git add f && git commit -m A
  git checkout -b b && echo B >f && git commit -am B
  git checkout -b c main && echo C >f && git commit -am C
  git checkout -q main
)

git-init-frozen
echo /sm-source >>.git/info/exclude
echo base >README && git add README
git -c protocol.file.allow=always submodule add ./sm-source sm
git commit -m "base"
git update-ref refs/heads/base HEAD

git checkout -b A
git -C sm checkout -q c
echo change-on-A >README
git add README sm && git commit -m "A-change"

git checkout main
git -C sm checkout -q b
echo change-on-main >README
git add README sm && git commit -m "main-change"
setup_target_to_match_main

git checkout A
git -C sm checkout -q c
create_workspace_commit_once A
