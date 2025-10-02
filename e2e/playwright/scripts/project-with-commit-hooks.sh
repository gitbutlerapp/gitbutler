#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $GITBUTLER_CLI_DATA_DIR"
echo "BUT_TESTING $BUT_TESTING"

# Setup a remote project
mkdir remote-with-hooks
pushd remote-with-hooks
git init -b master --object-format=sha1
echo "Initial content" >> initial_file.txt
git add initial_file.txt
git commit -am "Initial commit"
popd

# Clone the remote into a folder
git clone remote-with-hooks local-with-hooks
pushd local-with-hooks
  git checkout master

  # Create a commit-msg hook that modifies the message
  mkdir -p .git/hooks
  cat > .git/hooks/commit-msg << 'HOOK_EOF'
#!/bin/sh
# This hook modifies commit messages by adding a prefix
MESSAGE_FILE="$1"
ORIGINAL_MESSAGE=$(cat "$MESSAGE_FILE")

# If message contains "REJECT", reject it
if echo "$ORIGINAL_MESSAGE" | grep -q "REJECT"; then
  echo "Error: Commit message contains forbidden word REJECT"
  exit 1
fi

# If message contains "MODIFY", add a prefix
if echo "$ORIGINAL_MESSAGE" | grep -q "MODIFY"; then
  echo "[MODIFIED] $ORIGINAL_MESSAGE" > "$MESSAGE_FILE"
fi
HOOK_EOF

  chmod +x .git/hooks/commit-msg

  # Create a pre-commit hook that checks for forbidden content
  cat > .git/hooks/pre-commit << 'HOOK_EOF'
#!/bin/sh
# This hook checks staged files for forbidden content

# Get the list of staged files
STAGED_FILES=$(git diff --cached --name-only)

# Check each staged file for FORBIDDEN content
for file in $STAGED_FILES; do
  if [ -f "$file" ]; then
    if grep -q "FORBIDDEN" "$file"; then
      echo "Error: File $file contains FORBIDDEN content"
      exit 1
    fi
  fi
done

exit 0
HOOK_EOF

  chmod +x .git/hooks/pre-commit

  # Create a post-commit hook that can be triggered to fail
  cat > .git/hooks/post-commit << 'HOOK_EOF'
#!/bin/sh
# This hook checks if a marker file exists to trigger failure

if [ -f "FAIL_POST_COMMIT" ]; then
  echo "Error: Post-commit hook failed due to FAIL_POST_COMMIT marker"
  exit 1
fi

exit 0
HOOK_EOF

  chmod +x .git/hooks/post-commit

  # Add some uncommitted changes for testing
  echo "Uncommitted changes" >> uncommitted.txt

  # Add the project to GitButler
  $BUT_TESTING add-project --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"
popd
