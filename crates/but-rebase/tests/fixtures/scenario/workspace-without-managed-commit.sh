#!/bin/bash

set -eu -o pipefail

source "$(dirname "$0")/shared.sh"

git init

git commit --allow-empty -m "base"
git commit --allow-empty -m "one"

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

# The workspace branch tip is a plain commit, NOT a managed workspace commit.
git checkout -b gitbutler/workspace
git commit --allow-empty -m "just a normal commit"
