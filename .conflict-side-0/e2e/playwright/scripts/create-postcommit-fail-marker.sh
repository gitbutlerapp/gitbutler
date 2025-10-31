#!/bin/bash
# Create a marker file to trigger post-commit failure

pushd local-with-hooks
touch FAIL_POST_COMMIT
popd
