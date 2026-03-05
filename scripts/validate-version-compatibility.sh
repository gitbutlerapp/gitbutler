#!/bin/bash
# Validate that two versions of the CLI are compatible with a simple smoke test.
#
# We make few assertions here, we rely mostly on commands exiting non-zero if something goes wrong.
#
# Note that every `but` command is piping to cat. This is a li'l backwards compatible hack to ensure
# that we don't get any prompts, as the pipe is detected as not being a tty even in the very
# earliest versions of the CLI.

set -o errexit
set -o pipefail

old_version="$1"
new_version="$2"

if [ -z "$old_version" ] || [ -z "$new_version" ]; then
  echo "usage: validate-version-compatibility.sh <old_version> <new_version>"
  exit 1
fi

function smoke_test() {
  but setup | cat # we currently need a call to setup when moving between release and nightly, not entirely sure why
  echo "hello" >> test.txt
  but commit -m "Commit with version: $(but --version)" | cat
  but status | cat
}

function banner_message() {
  echo ""
  echo "### $1 ###" | tr '[:lower:]' '[:upper:]'
  echo ""
}

banner_message "Performing initial setup of '$old_version'"
echo "$(git rev-parse --show-toplevel)/scripts/install.sh" | bash -s "$old_version"
but config metrics disable

tmpdir=$(mktemp -d)
git clone https://github.com/gitbutlerapp/gitbutler "$tmpdir/test" --depth 100 # depth is pretty arbitrary, we just want a non-empty repo
cd "$tmpdir/test"

but setup | cat
but config user set name "Smoke Testingsson"
but config user set email "example@example.com"
but branch new test-branch
smoke_test

banner_message "Upgrading to '$new_version'"
but update install "$new_version" | cat
smoke_test

banner_message "Downgrading to '$old_version'"
but update install "$old_version" | cat
smoke_test

banner_message "Ensure versions are represented in commit messages"
num_old_version_commits=$(but status | grep "Commit with version:.*$old_version" | wc -l)
num_new_version_commits=$(but status | grep "Commit with version:.*$new_version" | wc -l)

echo "Found $num_old_version_commits commits with $old_version"
if [[ "$num_old_version_commits" -ne 2 ]]; then
  echo "Expected 2 commits with $old_version!"
  exit 1
fi

echo "Found $num_new_version_commits commits with $new_version"
if [[ "$num_new_version_commits" -ne 1 ]]; then
  echo "Expected 1 commits with $new_version!"
  exit 1
fi

banner_message "No obvious errors detected, test run successful!"
