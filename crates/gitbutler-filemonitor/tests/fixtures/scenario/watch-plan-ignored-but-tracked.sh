#!/usr/bin/env bash

### Description
# Repository with an ignored directory that contains tracked files, to verify watch-plan creation.

set -eu -o pipefail

git init

# These should already exist after `git init`, but keep the fixture deterministic.
mkdir -p .git/logs .git/refs/heads

cat >.gitignore <<'EOF'
ignored_dir/
EOF

mkdir -p ignored_dir
echo "tracked" >ignored_dir/tracked_file
git add -f ignored_dir/tracked_file

mkdir -p normal_dir
echo "hi" >normal_dir/file.txt
