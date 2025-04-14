#!/bin/bash

# This script runs deletes all generated fixtures before running the tests.
# This prevents potentially incorrect passes or failures due to stale fixtures.
# Especially considering that the time difference is negligible, this should be
# the preferred way to run tests.
#
# Time to run tests without fixtures present:
# ./scripts/cargo-test.sh  57.53s user 145.29s system 205% cpu 1:38.60 total
# Time to run tests with fixtures present:
# cargo test  46.74s user 176.52s system 288% cpu 1:17.31 total

./scripts/delete-fixtures.sh

echo "Running tests"
cargo test $@