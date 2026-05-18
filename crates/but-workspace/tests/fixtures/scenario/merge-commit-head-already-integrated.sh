#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init --initial-branch=master
echo "Branch whose head is a merge commit that merged master into feature - already integrated upstream" >.git/description

commit o1
git branch o1

# Feature branch with one commit
git checkout -b feature
commit feature-work
git branch feature-work

# Master advances
git checkout master
commit o2

# Feature merges master into itself (common pattern: "catch up with master")
git checkout feature
git merge --no-ff -m "Merge master into feature" master

# Master advances further and also merges the feature branch
git checkout master
git merge --no-ff -m "Merge feature" feature
git branch merged-point
commit o3

setup_remote_tracking master master
cat <<EOF >>.git/config
[remote "origin"]
	url = ./fake/local/path/which-is-fine-as-we-dont-fetch-or-push
	fetch = +refs/heads/*:refs/remotes/origin/*

[branch "master"]
	remote = "origin"
	merge = refs/heads/master
EOF

git checkout feature
create_workspace_commit_once feature
