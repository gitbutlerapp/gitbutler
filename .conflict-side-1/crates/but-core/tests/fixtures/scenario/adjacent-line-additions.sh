#!/usr/bin/env bash

### Description
# Minimal reproductions for partial-commit checkout conflicts.
# - `file`: two adjacent added lines; committing one conflicts with the other.
# - `file2`: a deleted line replaced by an inserted line; committing just
#   the deletion conflicts with the insertion.
set -eu -o pipefail

git init
cat <<'EOF' >file
line1
line2
line3
EOF
cat <<'EOF' >file2
line1
old-line
line3
EOF
git add . && git commit -m "init"

# Add two adjacent lines between line1 and line2
cat <<'EOF' >file
line1
added-a
added-b
line2
line3
EOF

# Replace old-line with new-line
cat <<'EOF' >file2
line1
new-line
line3
EOF
