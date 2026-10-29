#!/usr/bin/env bash

set -eu -o pipefail

git init

git checkout -b main
cat >story.txt <<'EOF'
line-1
line-2
line-3
line-4
line-5
line-6
line-7
line-8
line-9
EOF
git add story.txt
git commit -m "base story"

git checkout -b branch
# Modify two lines far enough apart that, with zero context lines, they diff as
# two separate hunks (`-2,1 +2,1` and `-8,1 +8,1`).
cat >story.txt <<'EOF'
line-1
EDIT-2
line-3
line-4
line-5
line-6
line-7
EDIT-8
line-9
EOF
git add story.txt
git commit -m "edit story"
