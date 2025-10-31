#!/bin/bash
# Create a file with allowed content

pushd local-with-hooks
echo "This is allowed content" > allowed.txt
popd
