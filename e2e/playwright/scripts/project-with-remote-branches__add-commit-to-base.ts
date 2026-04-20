import { logEnv, appendLine, pushd, popd, git } from "./lib.ts";

logEnv();

pushd("remote-project");
// Checkout branch 1
git("checkout", "master");
appendLine("a_file", "Update to main branch");
git("add", "b_file");
git("commit", "-am", "commit in base");
popd();
