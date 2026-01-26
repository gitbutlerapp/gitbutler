#!/usr/bin/env bash

### Description
# A commit with an executable, a normal file, a symlink.
# Then each item gets modified and renamed in the worktree, with one overwriting a tracked file and directory.
set -eu -o pipefail

git init
seq 5 8 >file
seq 9 13 >other-file
seq 1 3 >executable
chmod +x executable 2>/dev/null || true
if ln -s nonexisting-target link 2>/dev/null; then
  :
else
  printf '%s' nonexisting-target >link
fi

touch file-to-be-dir
mkdir dir-to-be-file && touch dir-to-be-file/content
touch to-be-overwritten

git add .
git update-index --chmod=+x executable
link_oid=$(printf '%s' nonexisting-target | git hash-object -w --stdin)
printf "120000 %s 0\tlink\n" "$link_oid" | git update-index --index-info
git commit -m "init"

seq 5 9 >file
seq 9 14 >other-file
seq 1 4 >executable
rm file-to-be-dir && mkdir file-to-be-dir && mv file file-to-be-dir/file
rm -Rf dir-to-be-file && mv executable dir-to-be-file
mv other-file to-be-overwritten

rm link
if ln -s other-nonexisting-target link-renamed 2>/dev/null; then
  :
else
  printf '%s' other-nonexisting-target >link-renamed
fi
link_renamed_oid=$(printf '%s' other-nonexisting-target | git hash-object -w --stdin)
printf "120000 %s 0\tlink-renamed\n" "$link_renamed_oid" | git update-index --index-info
