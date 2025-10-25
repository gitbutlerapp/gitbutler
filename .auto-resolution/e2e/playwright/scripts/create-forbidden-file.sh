#!/bin/bash
# Create a file with forbidden content

pushd local-with-hooks
echo "This contains FORBIDDEN content" > forbidden.txt
popd
