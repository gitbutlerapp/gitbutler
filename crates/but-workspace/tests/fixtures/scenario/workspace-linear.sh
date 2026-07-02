#!/bin/bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

echo "base" >base && git add . && git commit -m "base"
git update-ref refs/heads/base $(git rev-parse HEAD)
echo "a" >a && git add . && git commit -m "a"
git update-ref refs/heads/a $(git rev-parse HEAD)
echo "b" >b && git add . && git commit -m "b"
git update-ref refs/heads/b $(git rev-parse HEAD)
echo "c" >c && git add . && git commit -m "c"
git update-ref refs/heads/c $(git rev-parse HEAD)

create_workspace_commit_once main
