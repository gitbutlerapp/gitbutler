#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

init-repo-with-files-and-remote

git checkout -b other_stack
echo "this is a" > file
commit "add file"
echo "this is b" > file
commit "overwrite file with b"
echo "this is c" > file
commit "overwrite file with c"

git checkout -b other-top-series
echo "this is d" > file
commit "overwrite file with d"
echo "this is e" > file
commit "overwrite file with e"
echo "this is f" > file
commit "overwrite file with f"

git checkout -b my_stack main
echo "this is a" > file_2
git add file_2 && commit "add file_2"
echo "this is b" > file_2
commit "overwrite file_2 with b"
echo "this is c" > file_2
commit "overwrite file_2 with c"

git checkout -b top-series
echo "this is d" > file_2
commit "overwrite file_2 with d"
echo "this is e" > file_2
commit "overwrite file_2 with e"
echo "this is f" > file_2
commit "overwrite file_2 with f"

create_workspace_commit_once top-series other-top-series
