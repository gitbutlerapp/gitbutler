#!/usr/bin/env bash

### Description
# A newly initialized git repository with a single untracked file that uses CRLF line ending.
# Gitattributes are used to enforce LF line-endings in tracked files, like one would have on Windows.
set -eu -o pipefail

git init
echo -n $'1\r\n2\r\n' > not-yet-tracked
