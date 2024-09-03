#!/usr/bin/env bash
set -eu -o pipefail
CLI=${1:?The first argument is the GitButler CLI}

git init remote
(cd remote
  echo first > file
  git add . && git commit -m "init"
)

function init-virtual() {
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"
  $CLI branch create virtual
}

function pack-refs() {
    git pack-refs --prune --all
    git config core.precomposeUnicode false
}

function unpack-packed-refs() {
    local git_dir=$(git rev-parse --git-dir)
    local packed_refs_file="$git_dir/packed-refs"

    if [[ ! -f "$packed_refs_file" ]]; then
        echo "No packed-refs file found."
        return 1
    fi

    while IFS= read -r line; do
        # Skip comments and peel lines
        [[ "$line" =~ ^# ]] && continue
        [[ "$line" =~ ^\^ ]] && continue

        # Split line into hash and ref
        hash=$(echo "$line" | awk '{print $1}')
        ref=$(echo "$line" | awk '{print $2}')

        # Skip invalid lines
        [[ -z "$hash" || -z "$ref" ]] && continue

        # Create directory for the ref if it does not exist
        ref_dir=$(dirname "$git_dir/$ref")
        mkdir -p "$ref_dir"

        # Write the hash to the ref file
        echo "$hash" > "$git_dir/$ref"
    done < "$packed_refs_file"

    rm $packed_refs_file

    git config core.precomposeUnicode false
}


export GITBUTLER_CLI_DATA_DIR=../user/gitbutler/app-data
branch_count=300
git clone remote many-local
(cd many-local
  init-virtual

  for name in $(seq $branch_count); do
    git branch "$name"
  done
  unpack-packed-refs
)

git clone remote many-local-packed
(cd many-local-packed
  init-virtual

  for name in $(seq $branch_count); do
    git branch "$name"
  done
  pack-refs
)

git clone many-local many-local-tracked
(cd many-local-tracked
  init-virtual

  for name in $(seq $branch_count); do
    git branch --track "$name"
  done
  unpack-packed-refs
)

git clone many-local-packed many-local-tracked-packed
(cd many-local-tracked-packed
  init-virtual

  for name in $(seq $branch_count); do
    git branch --track "$name"
  done
  pack-refs
)
