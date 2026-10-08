#!/usr/bin/env bash

set -eu -o pipefail

git init -b main
git config user.name GitButler
git config user.email gitbutler@example.com
export GIT_AUTHOR_DATE="2000-01-01 00:00:00 +0000"
export GIT_COMMITTER_DATE="2000-01-02 00:00:00 +0000"

git commit --allow-empty -m "base"
git update-ref refs/remotes/origin/main HEAD

for line in {1..20}; do
    printf 'base line %s\n' "$line"
done >shared.txt
git add shared.txt
git commit -m "add shared file"

git checkout -b A
sed -i '1s/.*/target branch change/' shared.txt
git add shared.txt
git commit -m "change first line"

git checkout -b B
sed -i '10s/.*/later branch change/' shared.txt
git add shared.txt
git commit -m "change later line"

git checkout -b gitbutler/workspace B
