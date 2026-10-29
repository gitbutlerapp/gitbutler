#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

git checkout -b main
echo "base" >base.txt
git add base.txt
git commit -m "base"

git checkout -b stack-a
echo "a" >a.txt
git add a.txt
git commit -m "stack A adds file"

git checkout main
git checkout -b stack-b
echo "b" >b.txt
git add b.txt
git commit -m "stack B adds file"

create_workspace_commit_aggressively stack-a stack-b
