#!/bin/bash

set -eu -o pipefail

git init

git switch -c base
echo -e "a\nb\nc" > base-f && git add . && git commit -m "base"

git switch -c a
echo -e "a\nb\nc" > foo-f && git add . && git commit -m "a"

git switch -c b
echo -e "a\nx\nc" > foo-f && git add . && git commit -m "b"

git switch -c first-parent
echo -e "1\nx\nc" > foo-f && git add . && git commit -m "first-parent"

git switch b
git switch -c second-parent
echo -e "a\nx\n2" > foo-f && git add . && git commit -m "second-parent"

git switch a
git switch -c third-parent
echo -e "a\nx\nc" > base-f && git add . && git commit -m "third-parent"

git switch a
git switch -c to-pick
echo -e "a\nx\nc\nd" > base-f && git add . && git commit -m "to-pick"
