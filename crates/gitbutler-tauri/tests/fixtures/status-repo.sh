#!/bin/bash

# A fixture to create repositories that have a lot of different changes in one.
set -eu -o pipefail

git init
touch removed-in-worktree \
      removed-in-index \
      modified-in-worktree \
      modified-in-index \
      removed-in-index-changed-in-worktree \
      executable-bit-added \
      file-to-link \
      untracked

echo "content not to add to the index" >intent-to-add

git add . :!untracked :!intent-to-add
git add --intent-to-add intent-to-add
git commit -m "init"

rm removed-in-worktree
git rm removed-in-index

echo change-in-worktree >>modified-in-worktree
echo change-in-index >>modified-in-index
git add modified-in-index

git rm --cached removed-in-index-changed-in-worktree
echo worktree-change >>removed-in-index-changed-in-worktree

chmod +x executable-bit-added

echo content >added-to-index && git add added-to-index

rm file-to-link && ln -s link-target file-to-link

empty=$(git hash-object -w --stdin </dev/null)
a=$(echo "a" | git hash-object -w --stdin)
b=$(echo "b" | git hash-object -w --stdin)
git update-index --index-info <<EOF
100644 $empty 1	conflicting
100644 $a 2	conflicting
100644 $b 3	conflicting
EOF
