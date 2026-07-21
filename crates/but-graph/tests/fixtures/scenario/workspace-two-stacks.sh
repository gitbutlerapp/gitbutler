#!/bin/bash

set -eu -o pipefail

source "$(dirname "$0")/shared.sh"

git init

git commit --allow-empty -m "base"

# Two genuinely divergent stacks, each branching off the shared base.
git checkout -b stack-a
git commit --allow-empty -m "A1"
git commit --allow-empty -m "A2"

git checkout -b stack-b main
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

# Create workspace commit merging both stacks
id=$(git commit-tree HEAD^{tree} -p stack-a -p stack-b -m "GitButler Workspace Commit")
git checkout -b gitbutler/workspace
git reset --hard $id
