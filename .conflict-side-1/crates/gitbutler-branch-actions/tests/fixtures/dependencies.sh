#!/usr/bin/env bash
set -eu -o pipefail
CLI=${1:?The first argument is the GitButler CLI}

# Add change ID to a commit.
#
# Example usage:
# set_change_id adbb3234 "change-id-1" "my_stack"
set_change_id() {
    local commit_hash=$1
    local change_id=$2
    local branch_name=$3

    local change_id_key="gitbutler-change-id"
    local gitbutler_header_version_key="gitbutler-headers-version"
    local gitbutler_header_version_value="2"

    if [ -z "$commit_hash" ] || [ -z "$change_id" ] || [ -z "$branch_name" ]; then
        echo "Usage: set_change_id <commit_hash> <change_id> <branch_name>"
        return 1
    fi

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
        echo "$gitbutler_header_version_key $gitbutler_header_version_value"
        echo "$change_id_key $change_id"
        echo ""
        echo "$message"
    } > new_commit

    local new_commit_hash=$(git hash-object -t commit -w new_commit)

    git update-ref "refs/heads/$branch_name" "$new_commit_hash"

    rm new_commit
}



git init remote
(cd remote
  echo first > file
  git add . && git commit -m "init"
)

export GITBUTLER_CLI_DATA_DIR=../user/gitbutler/app-data
git clone remote independent-commits
(cd independent-commits
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

git clone remote complex-branch-checkout
(cd complex-branch-checkout
  git switch -c my_stack
  echo "this is a" > a
  git add a && git commit -m "add a" --trailer ""
  set_change_id "$(git rev-parse HEAD)" "change-id-1" "my_stack"

  echo "this is b" > b
  git add b && git commit -m "add b"
  set_change_id "$(git rev-parse HEAD)" "change-id-2" "my_stack"

  echo "this updates a" > a
  git add . && git commit -m "update a"
  set_change_id "$(git rev-parse HEAD)" "change-id-3" "my_stack"

  git switch -c delete-b
  rm -rf b
  git add . && git commit -m "delete b"
  set_change_id "$(git rev-parse HEAD)" "change-id-4" "delete-b"

  git checkout my_stack
  git merge delete-b --no-edit --no-ff
  git branch -D delete-b

  echo "this is c" > c
  git add c && git commit -m "add c"
  set_change_id "$(git rev-parse HEAD)" "change-id-5" "my_stack"

  echo "update a again" > a
  git add . && git commit -m "update a again"
  set_change_id "$(git rev-parse HEAD)" "change-id-6" "my_stack"

  git checkout main

  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"
  $CLI branch apply -b my_stack
)
