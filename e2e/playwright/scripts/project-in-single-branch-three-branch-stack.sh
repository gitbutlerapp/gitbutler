#!/bin/bash

set -euo pipefail

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

head_branch="${1:-C}"
empty_top_branch="${2:-false}"

# Setup a remote project. GitButler currently requires projects to have a remote.
mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1
echo "base" > base.txt
git add base.txt
git commit -m "base: initial commit"
popd

# Clone the remote, register the project with GitButler, configure the target,
# then create a local non-empty stack: master <- A <- B <- C.
git clone remote-project local-clone
pushd local-clone
git checkout master
target_branch="$(git rev-parse --symbolic-full-name @{u})"
target_branch="${target_branch#refs/remotes/}"
"$BUT" setup
"$BUT" config target "$target_branch"

git checkout -b A master
echo "A" > A.txt
git add A.txt
git commit -m "A: first commit"

git checkout -b B
echo "B" > B.txt
git add B.txt
git commit -m "B: first commit"

git checkout -b C
if [ "$empty_top_branch" != "true" ]; then
  echo "C" > C.txt
  git add C.txt
  git commit -m "C: first commit"
fi

python3 - .git/gitbutler/but.sqlite <<'PYTHON'
import sqlite3
import sys

with sqlite3.connect(sys.argv[1]) as database:
    # `but setup` created the schema; bind refs as bytes for its BLOB columns.
    database.execute(
        "DELETE FROM branch_order WHERE branch_ref_name IN (?, ?, ?)",
        (b"refs/heads/C", b"refs/heads/B", b"refs/heads/A"),
    )
    database.executemany(
        "INSERT INTO branch_order (branch_ref_name, parent_ref_name) VALUES (?, ?)",
        [
            (b"refs/heads/C", b"refs/heads/B"),
            (b"refs/heads/B", b"refs/heads/A"),
            (b"refs/heads/A", None),
        ],
    )
PYTHON

git checkout "$head_branch"
popd
