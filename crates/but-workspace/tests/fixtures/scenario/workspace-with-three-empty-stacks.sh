#!/bin/bash

set -eu -o pipefail

source "$(dirname "$0")/shared.sh"

git init

git commit --allow-empty -m "init"

# All three stacks point to the same base commit (init)
git update-ref refs/heads/stack-1 HEAD
git update-ref refs/heads/stack-2 HEAD
git update-ref refs/heads/stack-3 HEAD

# main advances with Commit X
git commit --allow-empty -m "Commit X"

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

# Create workspace commit merging all three stacks
id=$(git commit-tree HEAD^{tree} -p stack-1 -p stack-2 -p stack-3 -m "GitButler Workspace Commit")
git checkout -b gitbutler/workspace
git reset --hard $id
