#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

init-repo-with-files-and-remote

function set-change-id() {
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

git checkout -b my_stack
echo "this is a" > a
git add a && git commit -m "add a" --trailer ""
set-change-id "change-id-1" "my_stack"

echo "this is b" > b
git add b && git commit -m "add b"
set-change-id "change-id-2" "my_stack"

echo "this updates a" > a
git add . && git commit -m "update a"
set-change-id "change-id-3" "my_stack"

git checkout -b delete-b
rm -rf b
git add . && git commit -m "delete b"
set-change-id "change-id-4" "delete-b"

git checkout my_stack
git merge delete-b --no-edit --no-ff
git branch -D delete-b

echo "this is c" > c
git add c && git commit -m "add c"
set-change-id "change-id-5" "my_stack"

echo "update a again" > a
git add . && git commit -m "update a again"
set-change-id "change-id-6" "my_stack"

create_workspace_commit_once my_stack