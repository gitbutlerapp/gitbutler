#!/bin/bash

set -eu -o pipefail

source "$(dirname "$0")/shared.sh"

git init

git commit --allow-empty -m "init"
git commit --allow-empty -m "Commit B"
git commit --allow-empty -m "Commit A"
git update-ref refs/heads/stack-1 HEAD
git checkout -b stack-2 stack-1
git checkout -b stack-3 stack-1

# Setup remote tracking for main
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
id=$(git commit-tree HEAD^{tree} -p stack-1 -p stack-2 -p stack-3 -m "GitButler Workspace Commit")
git checkout -b gitbutler/workspace
git reset --hard $id