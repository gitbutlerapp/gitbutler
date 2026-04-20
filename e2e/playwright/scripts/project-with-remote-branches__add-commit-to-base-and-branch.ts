import { logEnv, appendLine, pushd, popd, git } from "./lib.ts";

logEnv();

pushd("remote-project");
// Checkout branch 1
git("checkout", "master");
appendLine("b_file", "create file b");
git("add", "b_file");
git("commit", "-am", "commit in base");

git("checkout", "branch1");
git("rebase", "master");
appendLine("b_file", "update file b");
git("add", "b_file");
git("commit", "-am", "branch1: update after base change");

git("checkout", "master");
popd();
