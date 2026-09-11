#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

# Local A changes the top of `f`; origin/A first changes the bottom of `f`, then
# carries the same top change rebased onto that. The file differs before and
# after in each copy, so only the replay itself can tell they are the same.
git-init-frozen
printf 'top\n\n\n\n\nbottom\n' >f && git add f && git commit -q -m "add f"
setup_target_to_match_main

printf 'TOP\n\n\n\n\nbottom\n' >f && git commit -q -am "change top"
git branch A
git reset --hard -q @~1

printf 'top\n\n\n\n\nBOTTOM\n' >f && git commit -q -am "change bottom"
printf 'TOP\n\n\n\n\nBOTTOM\n' >f && git commit -q -am "change top"
git update-ref refs/remotes/origin/A HEAD
git reset --hard -q @~2

create_workspace_commit_once main A
