import { logEnv, mkdir, appendLine, pushd, popd, git } from "./lib.ts";

logEnv();

// Create a simple repository to use as a submodule
mkdir("submodule-repo");
pushd("submodule-repo");
git("init", "-b", "main", "--object-format=sha1");
appendLine("submodule_file", "submodule content");
git("add", "submodule_file");
git("commit", "-m", "Initial submodule commit");
popd();

// Add a submodule to the local clone
pushd("local-clone");
git("submodule", "add", "../submodule-repo", "my-submodule");
popd();
