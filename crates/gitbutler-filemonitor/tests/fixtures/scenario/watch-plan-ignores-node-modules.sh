#!/usr/bin/env bash

### Description
# Repository with an ignored `node_modules/` directory, to verify watch-plan creation.

set -eu -o pipefail

git init submodule-repo
(cd submodule-repo
  mkdir dir
  echo content >dir/submodule-file
  git add . && git commit -m "init"
)

git init

cat >.gitignore <<'EOF'
node_modules/
EOF

mkdir -p src
echo "hi" >src/app.txt

mkdir -p node_modules/pkg
echo "x" >node_modules/pkg/index.js

git submodule add ./submodule-repo submodule-worktree

