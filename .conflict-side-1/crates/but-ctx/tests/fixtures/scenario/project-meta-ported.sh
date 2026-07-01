#!/usr/bin/env bash

### Description
# Project metadata is present in git config and legacy TOML, with differing values.

set -eu -o pipefail

git init
git config user.name "Test User"
git config user.email "test@example.com"
git remote add origin https://example.com/origin.git
git remote add upstream https://example.com/upstream.git

echo content >file
git add file
GIT_AUTHOR_DATE="2000-01-01 00:00:00 +0000" \
GIT_COMMITTER_DATE="2000-01-01 00:00:00 +0000" \
git commit -m "initial"

git update-ref refs/remotes/origin/main HEAD
git update-ref refs/remotes/upstream/trunk HEAD

mkdir -p .git/gitbutler
cat >.git/gitbutler/virtual_branches.toml <<EOF
[default_target]
branchName = "main"
remoteName = "origin"
remoteUrl = ""
sha = "$(git rev-parse HEAD)"
pushRemoteName = "fork"

[branch_targets]

[branches]
EOF

git config gitbutler.project.targetRef refs/remotes/upstream/trunk
git config gitbutler.project.targetCommitId "$(git rev-parse HEAD)"
git config gitbutler.project.pushRemote origin
git config gitbutler.project.portedMeta true
