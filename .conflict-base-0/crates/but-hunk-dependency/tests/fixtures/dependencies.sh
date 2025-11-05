#!/usr/bin/env bash
set -eu -o pipefail
CLI=${1:?The first argument is the GitButler CLI}

git init remote
(cd remote
  echo first > file
  git add . && git commit -m "init"
)

GITBUTLER_CHANGE_ID=0
function commit_stack() {
  local stack="${1:?}"
  local message="${2:?}"
  ((GITBUTLER_CHANGE_ID += 1))
  GITBUTLER_CHANGE_ID=$GITBUTLER_CHANGE_ID $CLI branch commit "$stack" -m "$message"
}

export GITBUTLER_CLI_DATA_DIR=../user/gitbutler/app-data
git clone remote independent-commits
(cd independent-commits
  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"


  $CLI branch create --set-default my_stack
  echo "this is a" >> a
  commit_stack "my_stack" "add a"
  echo "this is b" >> b
  commit_stack "my_stack" "add b"
  echo "this is c" >> c
  commit_stack "my_stack" "add c"

  $CLI branch series my_stack -s "top-series"
  echo "this is d" >> d
  commit_stack "my_stack" "add d"
  echo "this is e" >> e
  commit_stack "my_stack" "add e"
  echo "this is f" >> f
  commit_stack "my_stack" "add f"
)

git clone remote independent-commits-multi-stack
(cd independent-commits-multi-stack
  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"


  $CLI branch create --set-default other_stack
  echo "this is a" >> a
  commit_stack "other_stack" "add a"
  echo "this is b" >> b
  commit_stack "other_stack" "add b"
  echo "this is c" >> c
  commit_stack "other_stack" "add c"

  $CLI branch series other_stack -s "other-top-series"
  echo "this is d" >> d
  commit_stack "other_stack" "add d"
  echo "this is e" >> e
  commit_stack "other_stack" "add e"
  echo "this is f" >> f
  commit_stack "other_stack" "add f"


  $CLI branch create --set-default my_stack
  echo "this is g" >> g
  commit_stack "my_stack" "add g"
  echo "this is h" >> h
  commit_stack "my_stack" "add h"
  echo "this is i" >> i
  commit_stack "my_stack" "add i"

  $CLI branch series my_stack -s "top-series"
  echo "this is j" >> j
  commit_stack "my_stack" "add j"
  echo "this is k" >> k
  commit_stack "my_stack" "add k"
  echo "this is l" >> l
  commit_stack "my_stack" "add l"
)
  
git clone remote sequentially-dependent-commits
(cd sequentially-dependent-commits
  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"


  $CLI branch create --set-default my_stack
  echo "this is a" > file
  commit_stack "my_stack" "add file"
  echo "this is b" > file
  commit_stack "my_stack" "overwrite file with b"
  echo "this is c" > file
  commit_stack "my_stack" "overwrite file with c"

  $CLI branch series my_stack -s "top-series"
  echo "this is d" > file
  commit_stack "my_stack" "overwrite file with d"
  echo "this is e" > file
  commit_stack "my_stack" "overwrite file with e"
  echo "this is f" > file
  commit_stack "my_stack" "overwrite file with f"
)

git clone remote sequentially-dependent-commits-multi-stack
(cd sequentially-dependent-commits-multi-stack
  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"


  $CLI branch create --set-default other_stack
  echo "this is a" > file
  commit_stack "other_stack" "add file"
  echo "this is b" > file
  commit_stack "other_stack" "overwrite file with b"
  echo "this is c" > file
  commit_stack "other_stack" "overwrite file with c"

  $CLI branch series other_stack -s "other-top-series"
  echo "this is d" > file
  commit_stack "other_stack" "overwrite file with d"
  echo "this is e" > file
  commit_stack "other_stack" "overwrite file with e"
  echo "this is f" > file
  commit_stack "other_stack" "overwrite file with f"

  $CLI branch create --set-default my_stack
  echo "this is a" > file_2
  commit_stack "my_stack" "add file_2"
  echo "this is b" > file_2
  commit_stack "my_stack" "overwrite file_2 with b"
  echo "this is c" > file_2
  commit_stack "my_stack" "overwrite file_2 with c"

  $CLI branch series my_stack -s "top-series"
  echo "this is d" > file_2
  commit_stack "my_stack" "overwrite file_2 with d"
  echo "this is e" > file_2
  commit_stack "my_stack" "overwrite file_2 with e"
  echo "this is f" > file_2
  commit_stack "my_stack" "overwrite file_2 with f"
)

git clone remote delete-and-recreate-file-multi-stack
(cd delete-and-recreate-file-multi-stack
  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"


  $CLI branch create --set-default other_stack
  echo "this is a" > file
  commit_stack "other_stack" "add file"
  echo "this is b" > file
  commit_stack "other_stack" "overwrite file with b"
  rm -rf file
  commit_stack "other_stack" "remove file"

  $CLI branch series other_stack -s "other-top-series"
  echo "this is d" > file
  commit_stack "other_stack" "recreate file with d"
  rm -rf file
  commit_stack "other_stack" "remove file again"
  echo "this is f" > file
  commit_stack "other_stack" "recreate file with f"

  $CLI branch create --set-default my_stack
  echo "this is a" > file_2
  commit_stack "my_stack" "add file_2"
  rm -rf file_2
  commit_stack "my_stack" "remove file_2"
  echo "this is c" > file_2
  commit_stack "my_stack" "recreate file_2 with c"

  $CLI branch series my_stack -s "top-series"
  rm -rf file_2
  commit_stack "my_stack" "remove file_2 again"
  echo "this is e" > file_2
  commit_stack "my_stack" "recreate file_2 with e"
  rm -rf file_2
  commit_stack "my_stack" "remove file_2 one last time"
)

function make_complex_file_manipulation() {
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
  commit_stack "my_stack" "add file"
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
  commit_stack "my_stack" "modify line 5"
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
  commit_stack "my_stack" "file: delete lines 4, 5 and 6 | file_2: delete lines g, h and i"
  rm -rf file
  commit_stack "my_stack" "remove file"

  $CLI branch series my_stack -s "top-series"
  echo "1
2
3
7
8
9
" > file
  commit_stack "my_stack" "recreate file"
  echo "1
2
3
7
8
9
a
b
c" > file
  commit_stack "my_stack" "add lines a, b and c at the end"
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
  commit_stack "my_stack" "file: add lines d and e at the beginning | file_2: modify line 1"
}

git clone remote complex-file-manipulation
(cd complex-file-manipulation
  make_complex_file_manipulation
)

git clone remote complex-file-manipulation-with-worktree-changes
(cd complex-file-manipulation-with-worktree-changes
  make_complex_file_manipulation
  echo "d
updated line 3
updated line 4
updated line 5
7
8
9
a
b
c" > file

  echo "1
b
c
updated d
e
f
" > file_2
)

function make_complex_file_manipulation_multiple_hunks() {
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
  commit_stack "my_stack" "create file"
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
  commit_stack "my_stack" "modify lines 4 and 8"
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
  commit_stack "my_stack" "insert 2 lines after 2, modify line 4 and remove line 6"
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
  commit_stack "my_stack" "insert 1 line at the top and bottom, remove lines 3 and 4 and update line 7"
}

git clone remote complex-file-manipulation-multiple-hunks
(cd complex-file-manipulation-multiple-hunks
  make_complex_file_manipulation_multiple_hunks
)

git clone remote complex-file-manipulation-multiple-hunks-with-changes
(cd complex-file-manipulation-multiple-hunks-with-changes
  make_complex_file_manipulation_multiple_hunks
  echo "added at the top
1
aaaaa
aaaaa
3
update 4 again
5
aaaaa
9
update bottom
add another line
" > file
)


function set_change_id() {
    local change_id=${1:?}
    local branch_name=${2:?}

    local commit_hash="$(git rev-parse HEAD)"

    local tree=$(git cat-file -p "$commit_hash" | grep "^tree" | awk '{print $2}')
    local parent=$(git cat-file -p "$commit_hash" | grep "^parent" | awk '{print $2}')
    local author=$(git cat-file -p "$commit_hash" | grep "^author" | cut -d' ' -f2-)
    local committer=$(git cat-file -p "$commit_hash" | grep "^committer" | cut -d' ' -f2-)

    local message_start=$(git cat-file -p "$commit_hash" | grep -n '^$' | head -n 1 | cut -d: -f1)
    local message=$(git cat-file -p "$commit_hash" | tail -n +"$((message_start + 1))")

    {
        echo "tree $tree"
        if [ -n "$parent" ]; then
            echo "parent $parent"
        fi
        echo "author $author"
        echo "committer $committer"
        echo "gitbutler-headers-version 2"
        echo "gitbutler-change-id $change_id"
        echo ""
        echo "$message"
    } > new_commit

    local new_commit_hash=$(git hash-object -t commit -w new_commit)

    git update-ref "refs/heads/$branch_name" "$new_commit_hash"

    rm new_commit
}

git clone remote complex-branch-checkout
(cd complex-branch-checkout
  git switch -c my_stack
  echo "this is a" > a
  git add a && git commit -m "add a" --trailer ""
  set_change_id "change-id-1" "my_stack"

  echo "this is b" > b
  git add b && git commit -m "add b"
  set_change_id "change-id-2" "my_stack"

  echo "this updates a" > a
  git add . && git commit -m "update a"
  set_change_id "change-id-3" "my_stack"

  git switch -c delete-b
  rm -rf b
  git add . && git commit -m "delete b"
  set_change_id "change-id-4" "delete-b"

  git checkout my_stack
  git merge delete-b --no-edit --no-ff
  git branch -D delete-b

  echo "this is c" > c
  git add c && git commit -m "add c"
  set_change_id "change-id-5" "my_stack"

  echo "update a again" > a
  git add . && git commit -m "update a again"
  set_change_id "change-id-6" "my_stack"

  git checkout main

  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"
  $CLI branch apply -b my_stack
)


git init remote2
(cd remote2
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


# Add the project, make some changes.
git clone remote2 merge-commit
(cd merge-commit
  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"


  $CLI branch create --set-default my_stack
  echo "this is a" > a
  commit_stack "my_stack" "add a"

# Create a new commit on the remote.
(cd ../remote2
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
  git fetch origin
  ((GITBUTLER_CHANGE_ID += 1))
  GITBUTLER_CHANGE_ID=$GITBUTLER_CHANGE_ID $CLI integrate-upstream merge

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
  commit_stack "my_stack" "update line 8 and delete the line after 7"

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
)
