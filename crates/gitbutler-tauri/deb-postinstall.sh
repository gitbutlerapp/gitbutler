#!/bin/sh
# Postinstall script for the deb package.

# Print metrics notice on fresh install
if [ "$1" = "configure" ] && [ -z "$2" ]; then
  echo ""
  echo "GitButler uses metrics to help us improve our product."
  echo "You can configure metrics collection either in the GUI or via 'but config metrics'."
  echo "Privacy policy: https://gitbutler.com/privacy"
  echo ""
fi
