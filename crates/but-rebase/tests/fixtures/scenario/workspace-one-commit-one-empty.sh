#!/bin/bash

set -eu -o pipefail

source "$(dirname "$0")/shared.sh"

git init

# Create the base commit (merge_base)
git commit --allow-empty -m "base"

# stack-2 (empty) pointing to base
git update-ref refs/heads/stack-2 HEAD

# main branch diverges from base with an additional commit
git commit --allow-empty -m "main diverged"

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

# stack-1 with one commit on top of base (not main)
git checkout -b stack-1 stack-2
git commit --allow-empty -m "top"

# Create workspace commit merging both stacks
# Note: stack-2 (empty) is parent 1, stack-1 (with commit) is parent 2
id=$(git commit-tree HEAD^{tree} -p stack-2 -p stack-1 -m "GitButler Workspace Commit")
git checkout -b gitbutler/workspace
git reset --hard $id
