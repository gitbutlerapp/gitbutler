#!/bin/bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

git commit --allow-empty -m "base"

# A single stack of two dependent branches: branch-top sits on top of
# branch-bottom, which sits on base. Each branch owns two commits.
git checkout -b branch-bottom
git commit --allow-empty -m "bottom 1"
git commit --allow-empty -m "bottom 2"
git checkout -b branch-top
git commit --allow-empty -m "top 1"
git commit --allow-empty -m "top 2"

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

# Workspace commit on top of the single stack (its tip is branch-top).
id=$(git commit-tree HEAD^{tree} -p branch-top -m "GitButler Workspace Commit")
git checkout -b gitbutler/workspace
git reset --hard $id
