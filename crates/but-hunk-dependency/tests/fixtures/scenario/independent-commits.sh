#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

init-repo-with-files-and-remote

git checkout -b my_stack
echo "this is a" >> a
git add a && commit "add a"
echo "this is b" >> b
git add b && commit "add b"
echo "this is c" >> c
git add c && commit "add c"

git checkout -b top-series
echo "this is d" >> d
git add d && commit "add d"
echo "this is e" >> e
git add e && commit "add e"
echo "this is f" >> f
git add f && commit "add f"

create_workspace_commit_once top-series