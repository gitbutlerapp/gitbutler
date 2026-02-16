#!/bin/bash

set -eu -o pipefail

ssh-keygen -t rsa -b 2048 -C "test@example.com" -N "" -f signature.key

git init

git config gpg.format ssh
git config user.signingKey "$PWD/signature.key"
git config GitButler.signCommits true
echo "*.key*" >.gitignore

export "GIT_CONFIG_COUNT=2"
export "GIT_CONFIG_KEY_0=commit.gpgsign"
export "GIT_CONFIG_VALUE_0=true"
export "GIT_CONFIG_KEY_1=init.defaultBranch"
export "GIT_CONFIG_VALUE_1=main"

echo "base" >base && git add . && git commit -m "base"
git update-ref refs/heads/base $(git rev-parse HEAD)
echo "a" >a && git add . && git commit -m "a"
git update-ref refs/heads/a $(git rev-parse HEAD)
echo "b" >b && git add . && git commit -m "b"
git update-ref refs/heads/b $(git rev-parse HEAD)
echo "c" >c && git add . && git commit -m "c"
git update-ref refs/heads/c $(git rev-parse HEAD)
