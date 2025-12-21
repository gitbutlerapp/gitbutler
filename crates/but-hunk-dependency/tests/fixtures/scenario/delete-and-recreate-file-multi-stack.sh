#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

init-repo-with-files-and-remote

git checkout -b other_stack
echo "this is a" > file
commit "add file"
echo "this is b" > file
commit "overwrite file with b"
git rm file
commit "remove file"

git checkout -b other-top-series
echo "this is d" > file
git add file && commit "recreate file with d"
git rm file
commit "remove file again"
echo "this is f" > file
git add file && commit "recreate file with f"

git checkout -b my_stack main
echo "this is a" > file_2
git add file_2 && commit "add file_2"
git rm file_2
commit "remove file_2"
echo "this is c" > file_2
git add file_2 && commit "recreate file_2 with c"

git checkout -b top-series
git rm file_2
commit "remove file_2 again"
echo "this is e" > file_2
git add file_2 && commit "recreate file_2 with e"
git rm file_2
commit "remove file_2 one last time"

create_workspace_commit_once top-series other-top-series
