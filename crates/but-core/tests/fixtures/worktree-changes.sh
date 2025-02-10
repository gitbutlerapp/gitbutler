#!/bin/bash

set -eu -o pipefail

git init untracked-unborn
(cd untracked-unborn
  touch untracked
)

git init added-unborn
(cd added-unborn
  touch untracked && git add untracked
)

git init added-modified-in-worktree
(cd added-modified-in-worktree
  touch modified intent-to-add
  echo something >modified
  git add . && git commit -m "init"
  echo change >modified

  touch added intent-to-add
  echo content >intent-to-add
  git add added
  git add --intent-to-add intent-to-add
)

git init modified-in-index
(cd modified-in-index
  touch modified
  echo something >modified
  git add . && git commit -m "init"
  echo change >modified && git add modified
)

git init file-to-dir-in-worktree
(cd file-to-dir-in-worktree
  touch file-then-dir && git add file-then-dir && git commit -m "init"
  rm file-then-dir && mkdir file-then-dir && echo content >file-then-dir/new-file
)

cp -Rv file-to-dir-in-worktree file-to-dir-in-index
(cd file-to-dir-in-index
  git add .
)

git init dir-to-file-in-worktree
(cd dir-to-file-in-worktree
  mkdir dir-soon-file && touch dir-soon-file/file
  git add dir-soon-file && git commit -m "init"
  rm -Rf dir-soon-file
  echo content >dir-soon-file
)

cp -Rv dir-to-file-in-worktree dir-to-file-in-index
(cd dir-to-file-in-index
  git add .
)

git init deleted-in-worktree
(cd deleted-in-worktree
  touch deleted
  echo something >deleted
  git add . && git commit -m "init"
  rm deleted
)

git init deleted-in-index
(cd deleted-in-index
  touch deleted
  echo something >deleted
  git add . && git commit -m "init"
  git rm deleted
)

git init renamed-in-index
(cd renamed-in-index
  echo content >to-be-renamed
  git add . && git commit -m "init"
  git mv to-be-renamed new-name
)

git init renamed-in-index-with-executable-bit
(cd renamed-in-index-with-executable-bit
  echo content >to-be-renamed && chmod +x to-be-renamed
  git add . && git commit -m "init"
  git mv to-be-renamed new-name
)

git init renamed-in-worktree
(cd renamed-in-worktree
  echo content >to-be-renamed
  git add . && git commit -m "init"
  mv to-be-renamed new-name
)

git init renamed-in-worktree-with-executable-bit
(cd renamed-in-worktree-with-executable-bit
  echo content >to-be-renamed && chmod +x to-be-renamed
  git add . && git commit -m "init"
  mv to-be-renamed new-name
)

git init modified-in-index-and-worktree
(cd modified-in-index-and-worktree
  echo initial >dual-modified
  git add . && git commit -m "init"
  echo change >>dual-modified && git add dual-modified
  echo second-change >>dual-modified
)

git init submodule-added-unborn
(cd submodule-added-unborn
  git submodule add ../modified-in-index submodule
)

git init submodule-changed-head
(cd submodule-changed-head
  git submodule add ../modified-in-index submodule
  git commit -m "init"
  (cd submodule
    echo change >>modified && git commit -am "change in submodule to adjust its HEAD ref"
  )
)

git init case-folding-worktree-changes
(cd case-folding-worktree-changes
  git config core.ignorecase false
  empty_oid=$(git hash-object -w --stdin </dev/null)
  other_oid=$(echo content | git hash-object -w --stdin)
  git update-index --index-info <<EOF
100644 $empty_oid	file
100644 $other_oid	FILE
EOF
  git commit -m "init with two files that fold into one on case-insensitive filesystems"
  git checkout -f HEAD
)

cp -Rv case-folding-worktree-changes case-folding-worktree-and-index-changes
(cd case-folding-worktree-and-index-changes
  empty_oid=$(git hash-object -w --stdin </dev/null)
  git update-index --index-info <<EOF
100644 $empty_oid	FILE
EOF
)

git init conflicting
(cd conflicting
  touch unrelated && git add . && git commit -m "init"

  empty=$(git hash-object -w --stdin </dev/null)
  a=$(echo "a" | git hash-object -w --stdin)
  b=$(echo "b" | git hash-object -w --stdin)
  git update-index --index-info <<EOF
100644 $empty 1	conflicting
100644 $a 2	conflicting
100644 $b 3	conflicting
EOF
)

git init big-file-20-unborn
(cd big-file-20-unborn
  seq 10 >big
)

git init binary-file-unborn
(cd binary-file-unborn
  printf '\0hi\0' >with-null-bytes
)

git init diff-binary-to-text-unborn
(cd diff-binary-to-text-unborn
  printf '\0hi\0' >file.binary
  echo "*.binary diff=say-hi" >.gitattributes

cat <<EOF >>.git/config
[diff "say-hi"]
	textconv = "shift; echo hi"
EOF
)

git init diff-binary-to-text-renamed-in-worktree
(cd diff-binary-to-text-renamed-in-worktree
  printf '\0hi\0' >before-rename.binary
  echo "before-rename.binary diff=say-hi" >.gitattributes
  echo "after-rename.binary diff=say-ho" >>.gitattributes
  git add .
  mv before-rename.binary after-rename.binary

cat <<EOF >>.git/config
[diff "say-hi"]
	textconv = "shift; echo hi"
[diff "say-ho"]
	textconv = "shift; echo ho"
EOF
)


git init -q sparse
(cd sparse
  touch a b
  mkdir c1
  (cd c1 && touch a b && mkdir c2 && cd c2 && touch a b)
  (cd c1 && mkdir c3 && cd c3 && touch a b)
  mkdir d
  (cd d && touch a b && mkdir c4 && cd c4 && touch a b c5)

  git add .
  git commit -m "init"

  git sparse-checkout set c1/c2 --sparse-index
)

