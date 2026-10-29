#!/usr/bin/env bash
set -eu -o pipefail

# The installer exports only local configuration, including its selected includes,
# in Git's record order. Global remote URLs must not enable local hasconfig includes.
git init -b main values
cat >>values/.git/config <<'EOF'
[GitButler]
	Repeated = first
	Empty =
	Escaped = "line\n\ttab\\\""
[core]
	ignoredImplicit
[include]
	path = local.inc
[gitbutler "MiXeD.quoted\"name"]
	Value = subsection
[gitbutler.Legacy]
	Value = legacy
[includeIf "gitdir:*/values/.git"]
	path = gitdir.inc
[includeIf "onbranch:main"]
	path = branch.inc
[includeIf "hasconfig:remote.*.url:https://local.example/**"]
	path = local-url.inc
[includeIf "hasconfig:remote.*.url:https://global.example/**"]
	path = global-url.inc
[remote "MiXeD"]
	URL = https://local.example/repo
	fetch = +refs/heads/*:refs/remotes/MiXeD/*
	fetch = +refs/tags/*:refs/tags/*
[log]
	excludeDecoration = refs/gitbutler/*
	excludeDecoration = refs/heads/private/*
	ignored = omitted
[log "other"]
	excludeDecoration = omitted
[gitbutler]
	Repeated = last
EOF
cat >values/.git/local.inc <<'EOF'
[gitbutler]
	Repeated = included
	Continued = one\
two
[include]
	path = nested.inc
EOF
printf '[gitbutler "bytes\377"]\n\tValue = "raw\376"\n' >values/.git/nested.inc
for condition in gitdir branch local-url global-url; do
  printf '[gitbutler]\n\tIncluded = %s\n' "$condition" >"values/.git/$condition.inc"
done
git -C values config --local --includes --null --get-regexp \
  '^(gitbutler\.|remote\.|log\.excludedecoration$)' >expected-config
cat >>values/.git/config <<'EOF'
[GitButler]
	storagePath = /do-not-export
[gitbutler "nightly"]
	storagePath = /do-not-export
[gitbutler "dev"]
	storagePath = /do-not-export
EOF

git init -b main paths
cat >>paths/.git/config <<'EOF'
[remote "Relative"]
	url = ../remote.git
	pushurl = ../push.git
[remote "Absolute"]
	url = /absolute/remote.git
EOF

# An implicit selected value is invalid even when another value has the same key.
for position in first last storage; do
  git init -b main "implicit-$position"
done
printf '[gitbutler]\nvalue\nvalue = explicit\n' >>implicit-first/.git/config
printf '[gitbutler]\nvalue = explicit\nvalue\n' >>implicit-last/.git/config
printf '[gitbutler]\nstoragePath\n' >>implicit-storage/.git/config

git init -b main empty
cat >global.config <<'EOF'
[remote "global"]
	url = https://global.example/repo
[gitbutler]
	global = omitted
EOF
