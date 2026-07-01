#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

init-repo-with-files-and-remote

git checkout -b other_stack
echo "this is a" >> a
git add a && commit "add a"
echo "this is b" >> b
git add b && commit "add b"
echo "this is c" >> c
git add c && commit "add c"

git checkout -b other-top-series
echo "this is d" >> d
git add d && commit "add d"
echo "this is e" >> e
git add e && commit "add e"
echo "this is f" >> f
git add f && commit "add f"


git checkout -b my_stack main
echo "this is g" >> g
git add g && commit "add g"
echo "this is h" >> h
git add h && commit "add h"
echo "this is i" >> i
git add i && commit "add i"

git checkout -b top-series
echo "this is j" >> j
git add j && commit "add j"
echo "this is k" >> k
git add k && commit "add k"
echo "this is l" >> l
git add l && commit "add l"

create_workspace_commit_once top-series other-top-series
