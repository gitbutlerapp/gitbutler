#!/bin/bash

set -eu -o pipefail

source "$(dirname "$0")/shared.sh"

git init

git commit --allow-empty -m "base"

# stack-a branches off base.
git checkout -b stack-a
git commit --allow-empty -m "A1"
git commit --allow-empty -m "A2"

# stack-b is a completely disjoint, orphan history (shares nothing with base).
git checkout --orphan stack-b
git rm -rf . >/dev/null 2>&1 || true
git commit --allow-empty -m "B1"
git commit --allow-empty -m "B2"

# Setup remote tracking for main (the target lives at base)
mkdir -p .git/refs/remotes/origin
cp .git/refs/heads/main .git/refs/remotes/origin/main

cat <<EOF >>.git/config
[remote "origin"]
	url = ./fake/local/path/which-is-fine-as-we-dont-fetch-or-push
	fetch = +refs/heads/*:refs/remotes/origin/*

[branch "main"]
  remote = "origin"
  merge = refs/heads/main
EOF

# Workspace commit merges the two disjoint stack tips.
id=$(git commit-tree HEAD^{tree} -p stack-a -p stack-b -m "GitButler Workspace Commit")
git checkout -b gitbutler/workspace
git reset --hard $id
