#!/bin/bash

set -eu -o pipefail

ssh-keygen -t rsa -b 2048 -C "test@example.com" -N "" -f signature.key

git init

git config gpg.format ssh
git config user.signingKey "$PWD/signature.key"
git config GitButler.signCommits true
echo "*.key*" >.gitignore

# Appended to git's command-scope config, which outranks the harness's commit.gpgsign=false
# however gix-testtools passes it.
export GIT_CONFIG_PARAMETERS="${GIT_CONFIG_PARAMETERS:+$GIT_CONFIG_PARAMETERS }'commit.gpgsign=true'"

echo "base" >base
git add .
git commit -m "base"
