#!/bin/bash

set -eu -o pipefail

git init

echo "base" >base && git add . && git commit -m "base"
# common ancestor
echo "a" >a && git add . && git commit -m "a"
git update-ref refs/heads/foo HEAD
# extra mutable ref
echo "b" >b && git add . && git commit -m "b"
git update-ref refs/heads/explicit-mut HEAD
# included implicitly as immutable
echo "c" >c && git add . && git commit -m "c"
git update-ref refs/heads/implicit-const HEAD
# extra immutable ref
echo "d" >d && git add . && git commit -m "d"
git update-ref refs/heads/explicit-const HEAD
git checkout --detach refs/heads/foo
# head ref
echo "e" >e && git add . && git commit -m "e"
git update-ref refs/heads/implicit-mut HEAD
# included implicitly as immutable
echo "f" >f && git add . && git commit -m "f"
git update-ref refs/heads/implicit-const-2 HEAD
# extra immutable ref
echo "g" >g && git add . && git commit -m "g"
git update-ref refs/heads/explicit-const-2 HEAD
git checkout refs/heads/implicit-mut
