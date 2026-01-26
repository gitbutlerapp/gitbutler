#!/usr/bin/env bash

### Description
# A newly initialized git repository with an executable, a normal file, a symlink and a fifo, added to the index.
set -eu -o pipefail

git init
echo content > untracked
echo exe > untracked-exe
chmod +x untracked-exe 2>/dev/null || true
if ln -s untracked link 2>/dev/null; then
  :
else
  printf '%s' untracked >link
fi

git add untracked untracked-exe link
git update-index --chmod=+x untracked-exe
link_oid=$(printf '%s' untracked | git hash-object -w --stdin)
printf "120000 %s 0\tlink\n" "$link_oid" | git update-index --index-info
