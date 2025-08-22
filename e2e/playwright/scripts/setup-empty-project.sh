#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $GITBUTLER_CLI_DATA_DIR"
echo "BUT_TESTING $BUT_TESTING"

# Setup a remote project.
# GitButler currently requires projects to have a remote
mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1
echo "foo\nbar\nbaz" > a_file
git add a_file
git commit -am "Hey, look! A commit."
popd

# Clone the remote into a folder.
# This is what we are going to add in the client
git clone remote-project local-clone
