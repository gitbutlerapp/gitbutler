#!/usr/bin/env bash

set -eu -o pipefail

git init
echo "feature carries two commits above the target, and base.txt is dirty in the worktree" >.git/description

git config user.name GitButler
git config user.email gitbutler@example.com

echo base >base.txt
git add base.txt && git commit -m "base"

mkdir -p .git/refs/remotes/origin
cp .git/refs/heads/main .git/refs/remotes/origin/main

cat <<EOF >>.git/config
[remote "origin"]
	url = ./fake/local/path/which-is-fine-as-we-dont-fetch-or-push
	fetch = +refs/heads/*:refs/remotes/origin/*
EOF

git checkout -b feature
echo first >first.txt
git add first.txt && git commit -m "first"
echo second >second.txt
git add second.txt && git commit -m "second"

# Pre-existing uncommitted work, unrelated to the commit that gets uncommitted.
echo "base edited in the worktree" >base.txt
