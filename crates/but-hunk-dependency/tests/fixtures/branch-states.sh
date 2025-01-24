#!/bin/bash

set -eu -o pipefail

git init 1-2-3-10
(cd 1-2-3-10
  seq 1 >file && git add file && git commit -m "one"
  seq 2 >file && git commit -am "two"
  seq 3 >file && git commit -am "three"
  seq 10 >file && git commit -am "to ten"
)

git clone 1-2-3-10 1-2-3-10_two
(cd 1-2-3-10_two
  sed -i '' 's/2/two/g' file
)

git clone 1-2-3-10 1-2-3-10-shift_two
(cd 1-2-3-10-shift_two
  {
    echo "shift-one\nshift-two";
    seq 10
  } >file && git commit -am "shift all by 2"
  sed -i '' 's/2/two/g' file
)

