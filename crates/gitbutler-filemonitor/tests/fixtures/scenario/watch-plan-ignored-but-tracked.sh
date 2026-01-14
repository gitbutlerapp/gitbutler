#!/usr/bin/env bash

### Description
# Repository with an ignored directory that contains tracked files, to verify watch-plan creation.

set -eu -o pipefail

git init submodule-repo
(cd submodule-repo
  mkdir dir
  echo content >dir/submodule-file
  git add . && git commit -m "init"
)

git init

git submodule add ./submodule-repo submodule-worktree

cat >.gitignore <<'EOF'
ignored_but_tracked_dir/
submodule-repo/
submodule-worktree/
EOF

mkdir -p ignored_but_tracked_dir
echo "tracked" >ignored_but_tracked_dir/tracked_file
git add -f ignored_but_tracked_dir/tracked_file

mkdir -p normal_dir
echo "hi" >normal_dir/file.txt

git commit -m "this creates a reflog folder"