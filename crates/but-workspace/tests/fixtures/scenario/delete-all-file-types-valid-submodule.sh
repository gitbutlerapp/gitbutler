#!/usr/bin/env bash

### Description
# A repository with all file-types, including a submodule, which are all scheduled for deletion.
# Notably we will keep the parent repository of the submodule so that it can be restored.
set -eu -o pipefail

git init embedded-repository
(cd embedded-repository
  echo content >file && git add . && git commit -m "init"
)

git init
git submodule add ./embedded-repository submodule
echo content >file-to-remain
echo exe >executable
chmod +x executable 2>/dev/null || true
if ln -s file-to-remain link 2>/dev/null; then
  :
else
  printf '%s' file-to-remain >link
fi
git add .
git update-index --chmod=+x executable
link_oid=$(printf '%s' file-to-remain | git hash-object -w --stdin)
printf "120000 %s 0\tlink\n" "$link_oid" | git update-index --index-info
git commit -m "init"

rm -Rf ./submodule/ executable link .gitmodules
