#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init --initial-branch=master
echo "Workspace stack whose merge parent was historically integrated upstream" >.git/description

commit o1
git branch o1

git checkout -b A
commit A

git checkout -b B
commit B

git checkout -b D A
commit D

git checkout -b C B
git merge --no-ff -m C D

git checkout -b E C
commit E

git checkout master
commit o2
git merge --no-ff -m o3 B
git branch o3
commit o4

setup_remote_tracking master master
cat <<EOF >>.git/config
[remote "origin"]
	url = ./fake/local/path/which-is-fine-as-we-dont-fetch-or-push
	fetch = +refs/heads/*:refs/remotes/origin/*

[branch "master"]
	remote = "origin"
	merge = refs/heads/master
EOF

git checkout E
create_workspace_commit_once E
