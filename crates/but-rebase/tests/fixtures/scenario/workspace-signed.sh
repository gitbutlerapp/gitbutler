#!/bin/bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

create_consistent_signing_key_at signature.key

git init

git config gpg.format ssh
git config user.signingKey "$PWD/signature.key"
git config GitButler.signCommits true
echo "*.key*" >.gitignore

# Appended to git's command-scope config, which outranks the harness's commit.gpgsign=false
# however gix-testtools passes it.
export GIT_CONFIG_PARAMETERS="${GIT_CONFIG_PARAMETERS:+$GIT_CONFIG_PARAMETERS }'commit.gpgsign=true'"

echo "base" >base && git add . && git commit -m "base"
git update-ref refs/heads/base $(git rev-parse HEAD)
echo "a" >a && git add . && git commit -m "a"
git update-ref refs/heads/a $(git rev-parse HEAD)
echo "b" >b && git add . && git commit -m "b"
git update-ref refs/heads/b $(git rev-parse HEAD)
echo "c" >c && git add . && git commit -m "c"
git update-ref refs/heads/c $(git rev-parse HEAD)

create_workspace_commit_once main
