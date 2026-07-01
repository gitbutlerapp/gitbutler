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
echo exe >dir/executable && chmod +x dir/executable
ln -s file-to-remain dir/link
git add . && git commit -m "init"

rm -Rf ./dir
echo dir-now-file >dir

