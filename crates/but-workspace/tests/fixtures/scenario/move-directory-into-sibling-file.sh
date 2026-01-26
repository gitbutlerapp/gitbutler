#!/usr/bin/env bash

### Description
# A directory with a file, an executable and a symlink is moved into a sibling file.
set -eu -o pipefail

git init
mkdir -p a/b
(cd a/b
  seq 5 8 >file
  seq 1 3 >executable
  chmod +x executable 2>/dev/null || true
  if ln -s nonexisting-target link 2>/dev/null; then
    :
  else
    printf '%s' nonexisting-target >link
  fi
)
seq 10 13 >a/sibling

git add .
git update-index --chmod=+x a/b/executable
link_oid=$(printf '%s' nonexisting-target | git hash-object -w --stdin)
printf "120000 %s 0\ta/b/link\n" "$link_oid" | git update-index --index-info
git commit -m "init"

rm a/sibling
mv a/b a/sibling

