#!/bin/bash

set -eu -o pipefail

git init

cat <<'EOF' >divergent.txt
shared-1
shared-2
shared-3
EOF
git add divergent.txt
git commit -m "shared-base"
git update-ref refs/heads/onto "$(git rev-parse HEAD)"

git branch local
git checkout local
cat <<'EOF' >divergent.txt
shared-1
shared-2
local-3
EOF
git add divergent.txt
git commit -m "local-change-line-three"
git update-ref refs/heads/local "$(git rev-parse HEAD)"

git checkout main
cat <<'EOF' >divergent.txt
shared-1
remote-2
shared-3
EOF
git add divergent.txt
git commit -m "remote-change-line-two"
git update-ref refs/heads/remote "$(git rev-parse HEAD)"
