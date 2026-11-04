#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

# Like an unlocked git-crypt repository: `*.secret` files go through a required filter whose
# Git form is binary. Stack A changes a.secret, and upstream changed b.secret.

git-init-frozen

cat >.git/secret-filter <<'EOF'
#!/usr/bin/env bash
set -eu -o pipefail
case "$1" in
  clean)
    printf 'SECRET\0'
    tr 'A-Za-z' 'N-ZA-Mn-za-m'
    ;;
  smudge)
    tail -c +8 | tr 'A-Za-z' 'N-ZA-Mn-za-m'
    ;;
esac
EOF
# Git runs filters in the worktree root, gix sets GIT_DIR.
git config filter.secret.clean 'bash "${GIT_DIR:-.git}/secret-filter" clean'
git config filter.secret.smudge 'bash "${GIT_DIR:-.git}/secret-filter" smudge'
git config filter.secret.required true

printf '*.secret filter=secret\n' >.gitattributes
printf 'base\n' >a.secret
printf 'base\n' >b.secret
git add .gitattributes a.secret b.secret
git commit -m "base"
setup_target_to_match_main
git remote set-url origin .

git checkout -b A
printf 'changed on A\n' >a.secret
git add a.secret
git commit -m "change a.secret"

create_workspace_commit_once A

git checkout main
printf 'changed upstream\n' >b.secret
git add b.secret
git commit -m "upstream change to b.secret"
git update-ref refs/remotes/origin/main main

git checkout gitbutler/workspace
