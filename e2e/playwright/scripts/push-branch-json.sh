#!/bin/bash

set -eu

branch="$1"
output="$2"
directory="${3:-local-clone}"

pushd "$directory"
"$BUT" push --json "$branch" > "$output"
popd
