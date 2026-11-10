#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# Like an unlocked git-crypt repository: `*.secret` files go through a required filter whose
# Git form is binary, and each clean call appends the path to .git/clean-calls. Stack A's commit
# changes a.secret and file.txt, and upstream changed file.txt too, so pulling makes A's commit
# conflicted. Nothing changes b.secret. Smudging fails while .git/fail-smudge exists.

git-init-frozen

cat >.git/secret-filter <<'EOF'
#!/usr/bin/env bash
set -eu -o pipefail
case "$1" in
  clean)
    echo "$2" >>"${GIT_DIR:-.git}/clean-calls"
    printf 'SECRET\0'
    tr 'A-Za-z' 'N-ZA-Mn-za-m'
    ;;
  smudge)
    if [ -e "${GIT_DIR:-.git}/fail-smudge" ]; then exit 1; fi
    tail -c +8 | tr 'A-Za-z' 'N-ZA-Mn-za-m'
    ;;
esac
EOF
# Git runs filters in the worktree root, gix sets GIT_DIR.
git config filter.secret.clean 'bash "${GIT_DIR:-.git}/secret-filter" clean %f'
git config filter.secret.smudge 'bash "${GIT_DIR:-.git}/secret-filter" smudge'
git config filter.secret.required true

printf '*.secret filter=secret\n' >.gitattributes
echo base > file.txt
printf 'base\n' >a.secret
printf 'base\n' >b.secret
git add .gitattributes file.txt a.secret b.secret
git commit -m "base"
git update-ref refs/heads/base HEAD

git checkout -b A
echo change-on-A > file.txt
printf 'changed on A\n' >a.secret
git add file.txt a.secret
git commit -m "A-change"

git checkout main
echo change-on-main > file.txt
git add file.txt
git commit -m "main-change"
setup_target_to_match_main

git checkout A
create_workspace_commit_once A
