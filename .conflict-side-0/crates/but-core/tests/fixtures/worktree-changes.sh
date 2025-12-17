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

git init modified-in-index-and-worktree-mod-mod
(cd modified-in-index-and-worktree-mod-mod
  echo initial >dual-modified
  git add . && git commit -m "init"
  echo change >>dual-modified && git add dual-modified
  echo second-change >>dual-modified
)

git init modified-in-index-and-worktree-mod-mod-noop
(cd modified-in-index-and-worktree-mod-mod-noop
  echo initial >dual-modified
  git add . && git commit -m "init"
  echo change >>dual-modified && git add dual-modified
  echo initial >dual-modified
)

git init modified-in-index-and-worktree-mod-mod-symlink
(cd modified-in-index-and-worktree-mod-mod-symlink
  ln -s nonexisting-initial link
  git add . && git commit -m "init"
  rm link && ln -s nonexisting-index link && git add .
  rm link && ln -s nonexisting-wt-change link
)

git init modified-in-index-and-worktree-mod-mod-symlink-noop
(cd modified-in-index-and-worktree-mod-mod-symlink-noop
  ln -s nonexisting-initial link
  git add . && git commit -m "init"
  rm link && ln -s nonexisting-index link && git add .
  rm link && ln -s nonexisting-initial link
)

git init modified-in-index-and-worktree-add-mod
(cd modified-in-index-and-worktree-add-mod
  echo initial >file
  git add .
  echo wt-change >>file
)

git init modified-in-index-and-worktree-add-del
(cd modified-in-index-and-worktree-add-del
  echo initial >file
  git add .
  rm file
)

git init modified-in-index-and-worktree-del-add
(cd modified-in-index-and-worktree-del-add
  echo initial >file && git add . && git commit -m "init"
  git rm file
  echo $'initial\nwt-changed' >file
)

git init modified-in-index-and-worktree-del-add-noop
(cd modified-in-index-and-worktree-del-add-noop
  echo initial >file && git add . && git commit -m "init"
  git rm file
  echo initial >file
)

git init modified-in-index-and-worktree-mod-del
(cd modified-in-index-and-worktree-mod-del
  echo initial >file && git add . && git commit -m "init"
  echo index >>file && git add .
  rm file
)

git init modified-in-index-and-worktree-rename-mod
(cd modified-in-index-and-worktree-rename-mod
  echo initial >file && git add . && git commit -m "init"
  git mv file file-renamed
  echo wt-change >>file-renamed
)

git init modified-in-index-and-worktree-rename-rename
(cd modified-in-index-and-worktree-rename-rename
  echo initial >file && git add . && git commit -m "init"
  git mv file file-renamed-in-index
  mv file-renamed-in-index file-renamed-in-wt
)

git init modified-in-index-and-worktree-rename-del
(cd modified-in-index-and-worktree-rename-del
  echo initial >file && git add . && git commit -m "init"
  git mv file file-renamed-in-index
  rm file-renamed-in-index
)

git init modified-in-index-and-worktree-mod-rename
(cd modified-in-index-and-worktree-mod-rename
  echo initial >file && git add . && git commit -m "init"
  echo index >>file && git add .
  echo wt-change >>file
  mv file file-renamed-in-wt
)

git init modified-in-index-and-worktree-rename-add
(cd modified-in-index-and-worktree-rename-add
  echo initial >file && git add . && git commit -m "init"
  git mv file file-renamed-in-index
  echo $'initial\nwt-change' >file
)

git init modified-in-index-and-worktree-add-rename
(cd modified-in-index-and-worktree-add-rename
  echo initial >file && git add .
  mv file file-renamed-in-wt
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

cp -Rv submodule-changed-head submodule-changed-head-ignore-all
(cd submodule-changed-head-ignore-all
  echo $'\tignore = all\n' >>.gitmodules
)

git init submodule-changed-worktree-ignore-none
(cd submodule-changed-worktree-ignore-none
  git submodule add ../modified-in-index submodule
  git commit -m "init"
  (cd submodule
    echo change >>modified
  )
  echo $'\tignore = none\n' >>.gitmodules && git commit -am "update .gitmodules"
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

git init non-utf8-encodings
(cd non-utf8-encodings
  printf '\x80\xc4\xc0' > windows1252
)
