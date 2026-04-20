import { logEnv, mkdir, appendLine, writeLine, pushd, popd, git } from "./lib.ts";

logEnv();

// Setup a remote project.
// GitButler currently requires projects to have a remote
mkdir("remote-project");
pushd("remote-project");
git("init", "-b", "master", "--object-format=sha1");
appendLine("a_file", "foo");
appendLine("a_file", "bar");
appendLine("a_file", "baz");
git("add", "a_file");
git("commit", "-am", "Hey, look! A commit.");
popd();

// Clone the remote into a folder.
// This is what we are going to add in the client
git("clone", "remote-project", "local-clone");

mkdir("not-a-git-repo");
pushd("not-a-git-repo");
writeLine("another_file", "I am not a git repository");
popd();

mkdir("empty-dir");
