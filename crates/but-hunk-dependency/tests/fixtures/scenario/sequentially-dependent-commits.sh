#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

init-repo-with-files-and-remote

git checkout -b my_stack
echo "this is a" > file
commit "add file"
echo "this is b" > file
commit "overwrite file with b"
echo "this is c" > file
commit "overwrite file with c"

git checkout -b top-series
echo "this is d" > file
commit "overwrite file with d"
echo "this is e" > file
commit "overwrite file with e"
echo "this is f" > file
commit "overwrite file with f"

create_workspace_commit_once top-series
