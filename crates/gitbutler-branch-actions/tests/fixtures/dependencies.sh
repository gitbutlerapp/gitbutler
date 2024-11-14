#!/usr/bin/env bash
set -eu -o pipefail
CLI=${1:?The first argument is the GitButler CLI}

git init remote
(cd remote
  echo first > file
  git add . && git commit -m "init"
)

export GITBUTLER_CLI_DATA_DIR=../user/gitbutler/app-data
git clone remote independent-commits
(cd independent-commits
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"


  $CLI branch create --set-default my_stack
  echo "this is a" >> a
  $CLI branch commit my_stack -m "add a"
  echo "this is b" >> b
  $CLI branch commit my_stack -m "add b"
  echo "this is c" >> c
  $CLI branch commit my_stack -m "add c"

  $CLI branch series my_stack -s "top-series"
  echo "this is d" >> d
  $CLI branch commit my_stack -m "add d"
  echo "this is e" >> e
  $CLI branch commit my_stack -m "add e"
  echo "this is f" >> f
  $CLI branch commit my_stack -m "add f"
)

git clone remote independent-commits-multi-stack
(cd independent-commits-multi-stack
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"


  $CLI branch create --set-default other_stack
  echo "this is a" >> a
  $CLI branch commit other_stack -m "add a"
  echo "this is b" >> b
  $CLI branch commit other_stack -m "add b"
  echo "this is c" >> c
  $CLI branch commit other_stack -m "add c"

  $CLI branch series other_stack -s "other-top-series"
  echo "this is d" >> d
  $CLI branch commit other_stack -m "add d"
  echo "this is e" >> e
  $CLI branch commit other_stack -m "add e"
  echo "this is f" >> f
  $CLI branch commit other_stack -m "add f"


  $CLI branch create --set-default my_stack
  echo "this is g" >> g
  $CLI branch commit my_stack -m "add g"
  echo "this is h" >> h
  $CLI branch commit my_stack -m "add h"
  echo "this is i" >> i
  $CLI branch commit my_stack -m "add i"

  $CLI branch series my_stack -s "top-series"
  echo "this is j" >> j
  $CLI branch commit my_stack -m "add j"
  echo "this is k" >> k
  $CLI branch commit my_stack -m "add k"
  echo "this is l" >> l
  $CLI branch commit my_stack -m "add l"
)

git clone remote sequentially-dependent-commits
(cd sequentially-dependent-commits
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"


  $CLI branch create --set-default my_stack
  echo "this is a" > file
  $CLI branch commit my_stack -m "add file"
  echo "this is b" > file
  $CLI branch commit my_stack -m "overwrite file with b"
  echo "this is c" > file
  $CLI branch commit my_stack -m "overwrite file with c"

  $CLI branch series my_stack -s "top-series"
  echo "this is d" > file
  $CLI branch commit my_stack -m "overwrite file with d"
  echo "this is e" > file
  $CLI branch commit my_stack -m "overwrite file with e"
  echo "this is f" > file
  $CLI branch commit my_stack -m "overwrite file with f"
)

git clone remote sequentially-dependent-commits-muli-stack
(cd sequentially-dependent-commits-muli-stack
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"


  $CLI branch create --set-default other_stack
  echo "this is a" > file
  $CLI branch commit other_stack -m "add file"
  echo "this is b" > file
  $CLI branch commit other_stack -m "overwrite file with b"
  echo "this is c" > file
  $CLI branch commit other_stack -m "overwrite file with c"

  $CLI branch series other_stack -s "other-top-series"
  echo "this is d" > file
  $CLI branch commit other_stack -m "overwrite file with d"
  echo "this is e" > file
  $CLI branch commit other_stack -m "overwrite file with e"
  echo "this is f" > file
  $CLI branch commit other_stack -m "overwrite file with f"

  $CLI branch create --set-default my_stack
  echo "this is a" > file_2
  $CLI branch commit my_stack -m "add file_2"
  echo "this is b" > file_2
  $CLI branch commit my_stack -m "overwrite file_2 with b"
  echo "this is c" > file_2
  $CLI branch commit my_stack -m "overwrite file_2 with c"

  $CLI branch series my_stack -s "top-series"
  echo "this is d" > file_2
  $CLI branch commit my_stack -m "overwrite file_2 with d"
  echo "this is e" > file_2
  $CLI branch commit my_stack -m "overwrite file_2 with e"
  echo "this is f" > file_2
  $CLI branch commit my_stack -m "overwrite file_2 with f"
)

git clone remote delete-and-recreate-file-multi-stack
(cd delete-and-recreate-file-multi-stack
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"


  $CLI branch create --set-default other_stack
  echo "this is a" > file
  $CLI branch commit other_stack -m "add file"
  echo "this is b" > file
  $CLI branch commit other_stack -m "overwrite file with b"
  rm -rf file
  $CLI branch commit other_stack -m "remove file"

  $CLI branch series other_stack -s "other-top-series"
  echo "this is d" > file
  $CLI branch commit other_stack -m "recreate file with d"
  rm -rf file
  $CLI branch commit other_stack -m "remove file again"
  echo "this is f" > file
  $CLI branch commit other_stack -m "recreate file with f"

  $CLI branch create --set-default my_stack
  echo "this is a" > file_2
  $CLI branch commit my_stack -m "add file_2"
  rm -rf file_2
  $CLI branch commit my_stack -m "remove file_2"
  echo "this is c" > file_2
  $CLI branch commit my_stack -m "recreate file_2 with c"

  $CLI branch series my_stack -s "top-series"
  rm -rf file_2
  $CLI branch commit my_stack -m "remove file_2 again"
  echo "this is e" > file_2
  $CLI branch commit my_stack -m "recreate file_2 with e"
  rm -rf file_2
  $CLI branch commit my_stack -m "remove file_2 one last time"
)

git clone remote complex-file-manipulation
(cd complex-file-manipulation
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"

  $CLI branch create --set-default my_stack
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
  echo "a
b
c
d
e
f
g
h
i
" > file_2
  $CLI branch commit my_stack -m "add file"
  echo "1
2
3
4
__update1__
6
7
8
9
" > file
  $CLI branch commit my_stack -m "modify line 5"
  echo "1
2
3
7
8
9
" > file
  echo "a
b
c
d
e
f
" > file_2
  $CLI branch commit my_stack -m "file: delete lines 4, 5 and 6 | file_2: delete lines g, h and i"
  rm -rf file
  $CLI branch commit my_stack -m "remove file"

  $CLI branch series my_stack -s "top-series"
  echo "1
2
3
7
8
9
" > file
  $CLI branch commit my_stack -m "recreate file"
  echo "1
2
3
7
8
9
a
b
c" > file
  $CLI branch commit my_stack -m "add lines a, b and c at the end"
  echo "d
e
1
2
3
7
8
9
a
b
c" > file
  echo "1
b
c
d
e
f
" > file_2
  $CLI branch commit my_stack -m "file: add lines d and e at the beginning | file_2: modify line 1"
)

git clone remote complex-file-manipulation-multiple-hunks
(cd complex-file-manipulation-multiple-hunks
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"

  $CLI branch create --set-default my_stack

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
  $CLI branch commit my_stack -m "create file"
  echo "1
2
3
update 4
5
6
7
update 8
9
" > file
  $CLI branch commit my_stack -m "modify lines 4 and 8"
  echo "1
2
insert line
insert line
3
update 4 again
5
7
update 8
9
" > file
  $CLI branch commit my_stack -m "insert 2 lines after 2, modify line 4 and remove line 6"
  echo "added at the top
1
2
3
update 4 again
5
update 7
update 8
9
added at the bottom
" > file
  $CLI branch commit my_stack -m "insert 1 line at the top and bottom, remove lines 3 and 4 and update line 7"
)