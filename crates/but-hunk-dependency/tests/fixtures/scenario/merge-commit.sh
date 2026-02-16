#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

  echo "1
2
3
4
5
6
7
8
9
" > file
git add file && commit "init"
setup-remote-and-vbtoml

git checkout -b my_stack
echo "this is a" > a
git add a && commit "add a"

git checkout -b tmp main
echo "1
2
3
4
update line 5
6
7
add this line
8
9
" > file
commit "update line 5 and add a line after 7"
remote-tracking-caught-up tmp main

git checkout my_stack
git merge --no-ff origin/main -m "merge 'origin/main' into 'my_stack'"
git branch -d tmp

echo "1
2
3
4
update line 5
6
7
update line 8
9
" > file
  commit "update line 8 and delete the line after 7"

  echo "1
2
3
4
update line 5 again
6
7
update line 8 again
9
" > file

create_workspace_commit_once my_stack