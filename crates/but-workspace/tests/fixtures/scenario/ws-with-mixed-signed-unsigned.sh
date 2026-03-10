#!/usr/bin/env bash

### Description
# A workspace with two stacks, where some commits are signed and others are not.
#
# Stack unsigned-*: A two-branch stack with all commits unsigned.
# Stack mixed: A two-branch stack with a mix of signed and unsigned commits.
#
# This is useful for testing commits_list_unsigned and commits_sign.

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

ssh-keygen -t rsa -b 2048 -C "test@example.com" -N "" -f signature.key

git init

git config gpg.format ssh
git config user.signingKey "$PWD/signature.key"
echo "*.key*" >.gitignore

commit M
setup_target_to_match_main

# Stack unsigned-*: two unsigned commits
git checkout -b unsigned-bottom
  echo "unsigned-bottom" > unsigned.txt && git add unsigned.txt && git commit -m "unsigned bottom commit"
git checkout -b unsigned-top
  echo "unsigned-top" >> unsigned.txt && git add unsigned.txt && git commit -m "unsigned top commit"

# Stack mixed: one unsigned and one signed commit
git checkout main -b mixed-bottom
  echo "mixed-bottom" > mixed.txt && git add mixed.txt && git commit -m "mixed unsigned bottom commit"
git checkout -b mixed-top
  echo "mixed-top" >> mixed.txt && git add mixed.txt && git commit -S -m "mixed signed top commit"

create_workspace_commit_once unsigned-top mixed-top
