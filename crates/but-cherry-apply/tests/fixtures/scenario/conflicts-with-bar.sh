#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

init-repo-with-files-and-remote

# Scenario 2: A commit that conflicts when cherry-picked onto bar (but not foo)

# Create foo stack - modifies foo.txt only
git checkout -b foo
echo "foo line 1" > foo.txt
commit "foo: add line 1"
echo "foo line 2" >> foo.txt
commit "foo: add line 2"

# Create bar stack - modifies bar.txt in a way that will conflict
git checkout -b bar main
echo "bar line 1" > bar.txt
commit "bar: overwrite content"
echo "bar line 2" >> bar.txt
commit "bar: add line 2"

# Create an unapplied branch with a commit that modifies bar.txt differently
git checkout -b bar-conflict-branch main
echo "conflicting bar change" > bar.txt
git add . && git commit -m "Conflicting change to bar.txt"
git tag bar-conflict

# Store this as a GitButler reference
git update-ref refs/gitbutler/bar-conflict bar-conflict

git checkout foo
create_workspace_commit_once foo bar
