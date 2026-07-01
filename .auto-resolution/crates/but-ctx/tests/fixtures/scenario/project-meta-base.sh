#!/usr/bin/env bash

### Description
# A repository with deterministic remote-tracking refs for project metadata tests.

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
