#!/bin/bash

set -eu -o pipefail

git init

cat <<'EOF' >divergent.txt
shared-1
shared-2
shared-3
EOF
git add divergent.txt
git commit -m "divergence-base"
git update-ref refs/heads/ancestor "$(git rev-parse HEAD)"

git branch local
git checkout local
cat <<'EOF' >divergent.txt
shared-1
local-2
shared-3
EOF
git add divergent.txt
git commit -m "local-change-middle-line"
git update-ref refs/heads/local "$(git rev-parse HEAD)"

git checkout main
cat <<'EOF' >divergent.txt
shared-1
remote-2
shared-3
EOF
git add divergent.txt
git commit -m "remote-change-middle-line"
git update-ref refs/heads/remote "$(git rev-parse HEAD)"
