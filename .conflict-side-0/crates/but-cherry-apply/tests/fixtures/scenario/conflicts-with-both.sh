#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

init-repo-with-files-and-remote

git checkout -b foo
echo "foo line 1" > foo.txt
commit "foo: add line 1"
echo "foo line 2" >> foo.txt
commit "foo: add line 2"

git checkout -b bar main
echo "bar line 1" > bar.txt
commit "bar: overwrite content"
echo "bar line 2" >> bar.txt
commit "bar: add line 2"

# Create an unapplied branch with a commit that modifies shared.txt in yet another way
git checkout -b both-conflict-branch main
echo "conflicting foo change" > foo.txt
echo "conflicting bar change" > bar.txt
git add . && git commit -m "Conflicting changes"
git tag both-conflict

# Store this as a GitButler reference
git update-ref refs/gitbutler/both-conflict both-conflict

git checkout foo
create_workspace_commit_once foo bar
