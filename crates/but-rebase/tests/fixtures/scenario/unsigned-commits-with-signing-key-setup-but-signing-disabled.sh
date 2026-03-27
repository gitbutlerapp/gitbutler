#!/bin/bash
#
# Description: Scenario with unsigned commits and commit signing disabled, but
# all the requisite configuration to sign commits is present.
#
# The scenario is designed to be used to test signing of commits in the absence
# of other changes.

set -eu -o pipefail

git init

ssh-keygen -t rsa -b 2048 -C "test@example.com" -N "" -f signature.key
git config gpg.format ssh
git config user.signingKey "$PWD/signature.key"
echo "*.key*" >.gitignore

echo "base" >base && git add . && git commit -m "base"
git update-ref refs/heads/base $(git rev-parse HEAD)
echo "mid" >mid && git add . && git commit -m "mid"
git update-ref refs/heads/mid $(git rev-parse HEAD)
echo "top" >top && git add . && git commit -m "top"
git update-ref refs/heads/top $(git rev-parse HEAD)
