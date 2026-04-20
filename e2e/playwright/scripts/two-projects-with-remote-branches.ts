import { logEnv, mkdir, appendLine, pushd, popd, git, gitOutputIn, butTestingIn } from "./lib.ts";

logEnv();

// Setup a remote project.
mkdir("remote-project");
pushd("remote-project");
git("init", "-b", "master", "--object-format=sha1");
appendLine("a_file", "foo");
appendLine("a_file", "bar");
appendLine("a_file", "baz");
git("add", "a_file");
git("commit", "-am", "Hey, look! A commit.");

// Create branch 1
git("checkout", "-b", "branch1");
appendLine("a_file", "branch1 commit 1");
git("commit", "-am", "branch1: first commit");
appendLine("a_file", "branch1 commit 2");
git("commit", "-am", "branch1: second commit");
git("checkout", "master");

// Create branch 2
git("checkout", "-b", "branch2");
appendLine("a_file", "branch2 commit 1");
git("commit", "-am", "branch2: first commit");
appendLine("a_file", "branch2 commit 2");
git("commit", "-am", "branch2: second commit");
git("checkout", "master");
popd();

// Clone the remote into a folder and add the project to the application.
git("clone", "remote-project", "local-clone");
pushd("local-clone");
git("checkout", "master");
const upstream1 = gitOutputIn(".", "rev-parse", "--symbolic-full-name", "@{u}");
butTestingIn(".", "add-project", "--switch-to-workspace", upstream1);
popd();

// Clone the remote into another folder and add the project as well.
git("clone", "remote-project", "local-clone-2");
pushd("local-clone-2");
git("checkout", "master");
const upstream2 = gitOutputIn(".", "rev-parse", "--symbolic-full-name", "@{u}");
butTestingIn(".", "add-project", "--switch-to-workspace", upstream2);
popd();
