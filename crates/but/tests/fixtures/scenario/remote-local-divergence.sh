#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen
commit-file M
commit-file only-on-remote
setup_target_to_match_main
git reset --hard @~1

commit-file only-on-local
create_workspace_commit_once main
# git log --decorate --oneline --graph gitbutler/workspace refs/remotes/origin/main >/tmp/x
# * df9a986 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
# * 643ade3 (main) add only-on-local
# | * 28baf9a (origin/main, origin/HEAD) add only-on-remote
# |/  
# * 0dc3733 add M

