#!/usr/bin/env bash
set -eu -o pipefail
CLI=${1:?The first argument is the GitButler CLI}

git init remote
(cd remote
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
  git add . && git commit -m "init"
)

export GITBUTLER_CLI_DATA_DIR=../user/gitbutler/app-data

# Add the project, make some changes.
git clone remote merge-commit
(cd merge-commit
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"


  $CLI branch create --set-default my_stack
  echo "this is a" > a
  $CLI branch commit my_stack -m "add a"
)

# Create a new commit on the remote.
(cd remote
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
  git add . && git commit -m "update line 5 and add a line after 7"
)

# Update the project.
(cd merge-commit
  git fetch origin
  $CLI integrate-upstream merge

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
  $CLI branch commit my_stack -m "update line 8 and delete the line after 7"
)