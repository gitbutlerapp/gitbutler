#!/usr/bin/env bash

git init
git config core.fileMode false
printf 'before\n' >script.sh
printf 'before\n' >added.sh
printf 'before\n' >removed.sh
printf 'before\n' >mode-only.sh
git add .
git update-index --chmod=+x script.sh
git update-index --chmod=+x removed.sh
git commit -m executable
git update-index --chmod=+x added.sh mode-only.sh
git update-index --chmod=-x removed.sh
printf 'after\n' >script.sh
printf 'after\n' >added.sh
printf 'after\n' >removed.sh

blob=$(git rev-parse HEAD:script.sh)
git update-index --index-info <<EOF
100755 $blob 1	modify-delete.sh
100755 $blob 3	modify-delete.sh
100644 $blob 1	base-preferred.sh
100755 $blob 3	base-preferred.sh
100755 $blob 1	ours-preferred.sh
100644 $blob 2	ours-preferred.sh
100755 $blob 3	ours-preferred.sh
100755 $blob 3	theirs-only.sh
EOF
for path in modify-delete.sh base-preferred.sh ours-preferred.sh theirs-only.sh untracked.sh; do
    printf 'after\n' >"$path"
    chmod +x "$path"
done
