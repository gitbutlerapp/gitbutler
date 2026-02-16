
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

function init-repo-with-files-and-remote () {
  git init
  echo "initial content" > shared.txt
  echo "foo content" > foo.txt
  echo "bar content" > bar.txt
  git add . && git commit -m "init"

  remote-tracking-caught-up main

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

function commit() {
  local message=${1:?first argument is the commit message}
  git commit -am "$message" --allow-empty
}

