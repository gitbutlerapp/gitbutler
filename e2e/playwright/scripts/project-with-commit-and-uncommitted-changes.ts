import { logEnv, mkdir, appendLine, pushd, popd, git } from "./lib.ts";

logEnv();

// Setup a remote project.
mkdir("remote-with-changes");
pushd("remote-with-changes");
git("init", "-b", "master", "--object-format=sha1");
appendLine("initial_file.txt", "Initial content");
git("add", "initial_file.txt");
git("commit", "-am", "Initial commit");
popd();

// Clone the remote into a folder
git("clone", "remote-with-changes", "local-with-changes");
pushd("local-with-changes");
git("checkout", "master");

// Add an extra commit on the main branch
appendLine("second_file.txt", "Second commit content");
git("add", "second_file.txt");
git("commit", "-am", "Second commit on main branch");

// Add some uncommitted changes
appendLine("uncommitted.txt", "Uncommitted changes");
appendLine("initial_file.txt", "Modified initial file");
popd();
