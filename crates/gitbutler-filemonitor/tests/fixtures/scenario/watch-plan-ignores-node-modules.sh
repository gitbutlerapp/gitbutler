#!/usr/bin/env bash

### Description
# Repository with an ignored `node_modules/` directory, to verify watch-plan creation.

set -eu -o pipefail

git init

# These should already exist after `git init`, but keep the fixture deterministic.
mkdir -p .git/logs .git/refs/heads

cat >.gitignore <<'EOF'
node_modules/
EOF

mkdir -p src
echo "hi" >src/app.txt

mkdir -p node_modules/pkg
echo "x" >node_modules/pkg/index.js

