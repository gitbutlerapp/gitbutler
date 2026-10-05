#!/usr/bin/env bash

set -eu -o pipefail

git init -b main
git config user.name GitButler
git config user.email gitbutler@example.com
export GIT_AUTHOR_DATE="2000-01-01 00:00:00 +0000"
export GIT_COMMITTER_DATE="2000-01-02 00:00:00 +0000"

git commit --allow-empty -m "base"
git update-ref refs/remotes/origin/main HEAD
git checkout -b feature

printf 'base\n' >renamed-old.txt
printf '\000\001old\377' >binary.dat
printf '#!/bin/sh\nprintf old\n' >mode.sh
non_utf8_path=$'invalid-\xff.txt'
printf 'old non-utf8 path\n' >"$non_utf8_path"
git add -A
git commit -m "add file types"

mv renamed-old.txt renamed-new.txt
printf '\000\002new\376' >binary.dat
chmod +x mode.sh
printf 'new non-utf8 path\n' >"$non_utf8_path"
