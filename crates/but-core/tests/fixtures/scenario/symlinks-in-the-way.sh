#!/usr/bin/env bash

set -eu -o pipefail

# `main` is checked out, `b` is the checkout target, and the worktree has a symlink where
# checkout may write or delete. Symlinks point into the ignored `elsewhere/`, so a write or
# delete through a symlink shows up there.
scenario=$1

git init -b main
printf 'elsewhere\n' >>.git/info/exclude
mkdir elsewhere
printf 'elsewhere\n' >elsewhere/x

commit() {
  git add -A
  git commit -q --allow-empty -m "$1"
}

case "$scenario" in
  ignored-symlinked-dir-where-target-adds-a-file)
    printf 'build\n' >>.git/info/exclude
    printf 'base\n' >base && commit A
    git switch -qc b
    mkdir build && printf 'tracked\n' >build/x && git add -f build/x && commit B
    git switch -q main
    ln -s elsewhere build
    ;;
  ignored-symlink-where-target-adds-a-file)
    printf 'x\n' >>.git/info/exclude
    printf 'base\n' >base && commit A
    git switch -qc b
    printf 'tracked\n' >x && git add -f x && commit B
    git switch -q main
    ln -s elsewhere/x x
    ;;
  untracked-symlinked-dir-where-target-adds-a-file)
    printf 'base\n' >base && commit A
    git switch -qc b
    mkdir new && printf 'tracked\n' >new/x && commit B
    git switch -q main
    ln -s elsewhere new
    ;;
  tracked-dir-turned-symlink-where-target-modifies-a-file)
    mkdir d && printf 'one\n' >d/x && commit A
    git switch -qc b
    printf 'two\n' >d/x && commit B
    git switch -q main
    rm -r d && ln -s elsewhere d
    ;;
  tracked-dir-turned-symlink-where-target-deletes-a-file)
    mkdir d && printf 'one\n' >d/x && printf 'keep\n' >keep && commit A
    git switch -qc b
    git rm -q d/x && commit B
    git switch -q main
    rm -r d && ln -s elsewhere d
    ;;
  tracked-file-turned-symlink-where-target-modifies-it)
    printf 'one\n' >x && commit A
    git switch -qc b
    printf 'two\n' >x && commit B
    git switch -q main
    rm x && ln -s elsewhere/x x
    ;;
  target-turns-a-dir-into-a-symlink)
    mkdir d && printf 'one\n' >d/x && commit A
    git switch -qc b
    git rm -rq d && ln -s elsewhere d && commit B
    git switch -q main
    ;;
  target-turns-a-symlink-into-a-dir)
    ln -s elsewhere d && commit A
    git switch -qc b
    git rm -q d && mkdir d && printf 'tracked\n' >d/x && commit B
    git switch -q main
    ;;
  target-keeps-a-symlink-and-modifies-a-file)
    ln -s elsewhere d && printf 'one\n' >f && commit A
    git switch -qc b
    printf 'two\n' >f && commit B
    git switch -q main
    ;;
  *)
    echo "unknown scenario: $scenario" >&2
    exit 1
    ;;
esac
