#!/usr/bin/env bash

set -euo pipefail

git init -b main local-clone
pushd local-clone
echo "line 1" >> main.txt
git add main.txt
git commit -m "add main.txt"

REMOTE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/test-remote-XXXXXX")"
trap 'rm -rf "$REMOTE_DIR"' EXIT
git init --bare "$REMOTE_DIR"
git remote add origin "$REMOTE_DIR"
git push -u origin main

git checkout -b feature-foo
echo "line 1" >> foo.txt
git add foo.txt
git commit -m "add foo.txt"
git push -u origin feature-foo

git switch --detach origin/feature-foo
echo "line 2" >> foo.txt
git add foo.txt
git commit -m "update foo.txt (remote)"
git push origin HEAD:feature-foo

git checkout main
"$BUT" setup
"$BUT" apply feature-foo
"$BUT" branch new empty-branch
popd
