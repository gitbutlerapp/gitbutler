#!/usr/bin/env bash

set -eu -o pipefail

git init

# Like git-crypt: a required, single-shot filter whose Git form is binary.
# Clean prepends a NUL-containing header and rot13s, smudge reverses it.
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
    tail -c +8 | tr 'A-Za-z' 'N-ZA-Mn-za-m'
    ;;
esac
EOF
# Git runs filters in the worktree root, gix sets GIT_DIR.
git config filter.secret.clean 'bash "${GIT_DIR:-.git}/secret-filter" clean %f'
git config filter.secret.smudge 'bash "${GIT_DIR:-.git}/secret-filter" smudge'
git config filter.secret.required true

printf '*.secret filter=secret\n' >.gitattributes
printf 'line1\nline2\nline3\n' >a.secret
printf 'b\n' >b.secret
git add .gitattributes a.secret b.secret
git commit -m "init"
