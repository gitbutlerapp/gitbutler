#!/usr/bin/env bash

set -eu -o pipefail

git init
git config core.autocrlf true

printf '1\r\n2\r\n3\r\n' >ImportOrdersJob.cs
git -c core.autocrlf=false add ImportOrdersJob.cs
git commit -m "legacy crlf blob"

printf '*.cs text eol=crlf\n' >.gitattributes
git add .gitattributes
git commit -m "add attributes"
