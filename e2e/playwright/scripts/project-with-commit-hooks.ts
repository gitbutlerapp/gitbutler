import { logEnv, mkdir, appendLine, pushd, popd, git, gitOutputIn, butTestingIn, writeHook } from "./lib.ts";

logEnv();

// Setup a remote project
mkdir("remote-with-hooks");
pushd("remote-with-hooks");
git("init", "-b", "master", "--object-format=sha1");
appendLine("initial_file.txt", "Initial content");
git("add", "initial_file.txt");
git("commit", "-am", "Initial commit");
popd();

// Clone the remote into a folder
git("clone", "remote-with-hooks", "local-with-hooks");
pushd("local-with-hooks");
git("checkout", "master");

// Create hooks directory
mkdir(".git/hooks");

// Create a commit-msg hook that modifies the message
writeHook(".git/hooks", "commit-msg", `#!/bin/sh
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
`);

// Create a pre-commit hook that checks for forbidden content
writeHook(".git/hooks", "pre-commit", `#!/bin/sh
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
`);

// Create a post-commit hook that can be triggered to fail
writeHook(".git/hooks", "post-commit", `#!/bin/sh
# This hook checks if a marker file exists to trigger failure

if [ -f "FAIL_POST_COMMIT" ]; then
  echo "Error: Post-commit hook failed due to FAIL_POST_COMMIT marker"
  exit 1
fi

exit 0
`);

// Add some uncommitted changes for testing
appendLine("uncommitted.txt", "Uncommitted changes");

// Add the project to GitButler
const upstream = gitOutputIn(".", "rev-parse", "--symbolic-full-name", "@{u}");
butTestingIn(".", "add-project", "--switch-to-workspace", upstream);
popd();
