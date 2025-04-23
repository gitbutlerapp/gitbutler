#!/usr/bin/env bash

### Description
# Two branches on top of a common base, one commit each, A puts 10 lines on top,
# B puts 10 lines to the bottom, no overlap.
# Everything is signed except for the merge commit.
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

seq 10 20 >file
git add . && git commit -m init

git branch B
git checkout -b A
seq 20 >file && git commit -am "add 10 to the beginning"

git checkout B
seq 10 30 >file && git commit -am "add 10 to the end"

git checkout -b merge
git -c commit.gpgsign=false merge A
