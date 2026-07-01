#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

function commit_file_with_message() {
  local file="${1:?first argument is the filename}"
  local content="${2:?second argument is the content}"
  local message="${3:?third argument is the commit message}"
  echo "$content" >"$file"
  git add "$file"
  git commit -m "$message"
}

git init --initial-branch=master
echo "Workspace stack whose merge parent was content-integrated upstream" >.git/description

commit o1
git branch o1

git checkout -b A
commit_file_with_message a.txt A A

git checkout -b B
commit_file_with_message b.txt B B

git checkout -b D A
commit D

git checkout -b C B
git merge --no-ff -m C D

git checkout -b E C
commit E

git checkout master
commit o2
git cherry-pick A
git cherry-pick B
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
