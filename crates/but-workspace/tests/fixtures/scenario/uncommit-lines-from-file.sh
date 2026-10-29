#!/usr/bin/env bash

set -eu -o pipefail

git init

git checkout -b main
cat >story.txt <<'EOF'
base-1
base-2
EOF
git add story.txt
git commit -m "base story"

git checkout -b branch
cat >story.txt <<'EOF'
base-1
base-2
keep-1
drop-1
drop-2
keep-2
EOF
git add story.txt
git commit -m "edit story"
