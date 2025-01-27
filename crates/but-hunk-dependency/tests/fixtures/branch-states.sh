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
  sed 's/2/two/g' <file >file.tmp
  mv file.tmp file
)

git clone 1-2-3-10 1-2-3-10_renamed-two
(cd 1-2-3-10_renamed-two
  git mv file file-renamed && git commit -m "rename file"
  sed 's/2/two/g' <file-renamed >file
  mv file file-renamed
)

git clone 1-2-3-10 1-2-3-10_add-five
(cd 1-2-3-10_add-five
  sed 's/5/5\n5\.5/g' <file >file.tmp
  mv file.tmp file
)

git clone 1-2-3-10 1-2-3-10_remove-five
(cd 1-2-3-10_remove-five
  sed '5d' <file >file.tmp
  mv file.tmp file
)

git clone 1-2-3-10 1-2-3-10-shift_two
(cd 1-2-3-10-shift_two
  {
    echo $'shift-one\nshift-two';
    seq 10
  } >file && git commit -am "shift all by 2"
  sed 's/2/two/g' <file >file.tmp
  mv file.tmp file
)

