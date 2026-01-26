#!/usr/bin/env bash

### Description
# A directory containing all file-types, and a submodule, is replaced with a single file.
set -eu -o pipefail

git init embedded-repository
(cd embedded-repository
  echo content >file && git add . && git commit -m "init"
)

git init
git submodule add ./embedded-repository dir/submodule
echo content >dir/file-to-remain
echo exe >dir/executable
chmod +x dir/executable 2>/dev/null || true
if ln -s file-to-remain dir/link 2>/dev/null; then
  :
else
  printf '%s' file-to-remain >dir/link
fi
git add .
git update-index --chmod=+x dir/executable
link_oid=$(printf '%s' file-to-remain | git hash-object -w --stdin)
printf "120000 %s 0\tdir/link\n" "$link_oid" | git update-index --index-info
git commit -m "init"

rm -Rf ./dir
echo dir-now-file >dir
