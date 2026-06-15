#!/usr/bin/env bash

set -eu -o pipefail

function tick() {
  if test -z "${tick+set}"; then
    tick=1675176957
  else
    tick=$(($tick + 60))
  fi
  GIT_COMMITTER_DATE="$tick +0100"
  GIT_AUTHOR_DATE="$tick +0100"
  export GIT_COMMITTER_DATE GIT_AUTHOR_DATE
}

function commit_file() {
  local name="${1:?First argument is the filename}"
  local content="${2:?Second argument is the content}"
  local message="${3:?Third argument is the commit message}"
  tick
  echo "$content" >"$name"
  git add "$name"
  git commit -m "$message"
}

git init
echo "feature and sibling branches off main, head on main" >.git/description

git config user.name GitButler
git config user.email gitbutler@example.com

commit_file shared.txt main "main"

mkdir -p .git/refs/remotes/origin
cp .git/refs/heads/main .git/refs/remotes/origin/main

cat <<EOF >>.git/config
[remote "origin"]
	url = ./fake/local/path/which-is-fine-as-we-dont-fetch-or-push
	fetch = +refs/heads/*:refs/remotes/origin/*
EOF

git checkout -b feature
commit_file feature.txt feature "feature"

git checkout main
git checkout -b sibling
commit_file sibling.txt sibling "sibling"

git checkout main
