import { logEnv, args, pushd, popd, git } from "./lib.ts";

logEnv();

const branch = args[0];
console.log(`BRANCH TO MERGE: ${branch}`);

// Merge the upstream branch into master and delete the upstream branch
pushd("remote-project");
git("checkout", "master");
git("merge", "--no-ff", "-m", `Merging upstream branch ${branch} into base`, branch);
git("branch", "-d", branch);
popd();
