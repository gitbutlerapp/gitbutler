#!/bin/bash

set -eu

branch="$1"
message="$2"
directory="${3:-local-clone}"

pushd "$directory"
"$BUT" commit --branch "$branch" --message "$message"
popd
