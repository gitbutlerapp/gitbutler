#!/bin/bash

set -eu -o pipefail

git init

printf 'base\n' >shared
printf 'unchanged\n' >unrelated
git add . && git commit -m "base"

printf 'middle\n' >shared
printf 'middle only\n' >middle-only
git add . && git commit -m "middle"
git branch middle

printf 'main only\n' >main-only
git add . && git commit -m "main"

git worktree add wt middle
git branch second middle
git worktree add wt2 second
