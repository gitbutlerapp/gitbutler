#!/bin/bash

echo "Pushing the first single-branch fixture commit to origin and setting upstream"

pushd local-clone
	git push -u origin HEAD~2:refs/heads/single-branch-fixture
	git branch --set-upstream-to=origin/single-branch-fixture single-branch-fixture
popd
