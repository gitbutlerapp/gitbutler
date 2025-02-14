#!/usr/bin/env bash

### Description
# A single branch with two signed commits. The first commit has 10 lines, the second adds
# another 10 lines to the top of the file.
# Large numbers are used make fuzzy-patch application harder.
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
git add file && git commit -m init && \
  git tag first-commit && git branch first-commit

seq 20  >file && git commit -am "insert 10 lines to the top"

