
# can only be called once per test setup
function create_workspace_commit_once() {
  local workspace_commit_subject="GitButler Workspace Commit"

  if [ $# == 1 ]; then
    local current_branch=$(git rev-parse --abbrev-ref HEAD)
    if [[ "$current_branch" != "$1" ]]; then
      echo "BUG: Must assure the current branch is the branch passed as argument: $current_branch != $1"
      return 42
    fi
  fi

  git checkout -b gitbutler/workspace
  if [ $# == 1 ] || [ $# == 0 ]; then
    git commit --allow-empty -m "$workspace_commit_subject"
  else
    git merge --no-ff -m "$workspace_commit_subject" "${@}"
  fi
}

function remote-tracking-caught-up () {
  local branch_name="${1:?}"
  local remote_branch_name=${2:-"$branch_name"}

  mkdir -p .git/refs/remotes/origin
  cp ".git/refs/heads/$branch_name" ".git/refs/remotes/origin/$remote_branch_name"
}

function setup-remote-and-vbtoml () {
  cat <<EOF >>.git/config
[remote "origin"]
  url = ./fake/local/path/which-is-fine-as-we-dont-fetch-or-push
  fetch = +refs/heads/*:refs/remotes/origin/*
EOF

  # Make sure the target is set.
  mkdir .git/gitbutler
  cat <<EOF >>.git/gitbutler/virtual_branches.toml
[default_target]
   branchName = "main"
   remoteName = "origin"
   remoteUrl = "."
   sha = "$(git rev-parse main)"
   pushRemoteName = "origin"

[branch_targets]

[branches]
EOF
}

function init-repo-with-files-and-remote () {
  git init
  echo "first" > file
  git add . && git commit -m "init"

  remote-tracking-caught-up main

  setup-remote-and-vbtoml
}

function commit() {
  local message=${1:?first argument is the commit message}
  git commit -am "$message" --allow-empty
}

function make-complex-file-manipulation-in-two-segments-no-workspace() {
  git checkout -b my_stack
  echo "1
2
3
4
5
6
7
8
9
" > file
  echo "a
b
c
d
e
f
g
h
i
" > file_2
  git add file_2 && commit "add file"
  echo "1
2
3
4
__update1__
6
7
8
9
" > file
  commit "modify line 5"
  echo "1
2
3
7
8
9
" > file
  echo "a
b
c
d
e
f
" > file_2
  git add file_2 && commit "file: delete lines 4, 5 and 6 | file_2: delete lines g, h and i"
  git rm file
  commit "remove file"

  git checkout -b top-series
  echo "1
2
3
7
8
9
" > file
  git add file && commit "recreate file"
  echo "1
2
3
7
8
9
a
b
c" > file
  commit "add lines a, b and c at the end"
  echo "d
e
1
2
3
7
8
9
a
b
c" > file
  echo "1
b
c
d
e
f
" > file_2
  commit "my_stack" "file: add lines d and e at the beginning | file_2: modify line 1"
}

function make-complex-file-manipulation-multiple-hunks-on-segment-no-workspace() {
  git checkout -b my_stack
  echo "1
2
3
4
5
6
7
8
9
" > file
  commit "create file"
  echo "1
2
3
update 4
5
6
7
update 8
9
" > file
  commit "modify lines 4 and 8"
  echo "1
2
insert line
insert line
3
update 4 again
5
7
update 8
9
" > file
  commit "insert 2 lines after 2, modify line 4 and remove line 6"
  echo "added at the top
1
2
3
update 4 again
5
update 7
update 8
9
added at the bottom
" > file
  commit "insert 1 line at the top and bottom, remove lines 3 and 4 and update line 7"
}
