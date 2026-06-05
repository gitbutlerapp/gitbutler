#!/usr/bin/env bash

set -e

git init

echo "line 1" >> main.txt
git add main.txt
git commit -m "add main.txt"

REMOTE_DIR="$(mktemp -d /tmp/test-remote-XXXXXX)"
git init --bare "$REMOTE_DIR"
git remote add origin "$REMOTE_DIR"
git push -u origin main

git checkout -b foo

echo "line 1" >> foo.txt
git add foo.txt
git commit -m "add foo.txt"

git checkout main

echo "line 1" >> main.txt
git add main.txt
git commit -m "update main.txt (1)"

git push -u origin main

but setup

but apply foo

but branch new bar

