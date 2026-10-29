#!/usr/bin/env bash
set -eu -o pipefail

# Shared normal-repository state used by dump tests that need tracked files,
# untracked visible files, ignored files/directories, and a Unicode root name.
git init "你好 repo"
(
  cd "你好 repo"
  printf "*.ignored\nignored-dir/\n" >.gitignore
  printf "tracked ignored" >tracked.ignored
  git add .gitignore
  git add -f tracked.ignored
  git commit -m "initial"
  mkdir -p .git/gitbutler
  printf "project_id = 'fixture'\n" >.git/gitbutler/vb.toml

  printf "visible" >visible.txt
  printf "#!/usr/bin/env sh\n" >executable.sh
  chmod +x executable.sh
  printf "ignored" >ignored.ignored
  mkdir ignored-dir
  printf "ignored dir" >ignored-dir/file.txt
)
