import { logEnv, appendLine, pushd, popd, git } from "./lib.ts";

logEnv();

pushd("remote-project");
// Checkout branch 1
git("checkout", "branch1");
appendLine("a_file", "branch1 commit 3");
git("commit", "-am", "branch1: third commit");

git("checkout", "master");
popd();
