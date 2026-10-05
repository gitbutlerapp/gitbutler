#!/usr/bin/env bash

set -eu -o pipefail

git init -b main
git config user.name GitButler
git config user.email gitbutler@example.com
export GIT_AUTHOR_DATE="2000-01-01 00:00:00 +0000"
export GIT_COMMITTER_DATE="2000-01-02 00:00:00 +0000"

printf 'base\nstable\n' >shared.txt
git add shared.txt
git commit -m "base"
git update-ref refs/remotes/origin/main HEAD

git checkout -b feature
printf 'feature\ntarget old\n' >shared.txt
git add shared.txt
git commit -m "feature content"

git checkout -b side main
printf 'side\nstable\n' >shared.txt
git add shared.txt
git commit -m "side content"

git checkout feature
if git merge --no-ff -m "merge side" side; then
	echo "expected the fixture merge to conflict" >&2
	exit 1
fi
printf 'resolved\ntarget old\n' >shared.txt
git add shared.txt
git commit -m "resolve side merge"

printf 'resolved\ntarget new\n' >shared.txt