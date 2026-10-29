#!/bin/bash

# Scenario: a linked worktree on `middle` whose `test.txt` has two uncommitted
# hunks. Used to check that amending only the first hunk into the worktree's
# branch leaves exactly the second hunk behind, without duplicating the first.

set -eu -o pipefail

git init

printf 'line 1\nline 2\nline 3\n' >test.txt
git add . && git commit -m "base"
git branch middle

printf 'main only\n' >main-only
git add . && git commit -m "main"

git worktree add wt middle
printf 'line 1\nline 1.1\nline 1.2\nline 2\nline 3\n' >wt/test.txt
