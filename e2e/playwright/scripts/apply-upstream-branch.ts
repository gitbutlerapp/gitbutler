import { logEnv, args, pushd, popd, butTestingIn } from "./lib.ts";

logEnv();

const branch = args[0];
const directory = args[1];

console.log(`BRANCH TO APPLY: ${branch}`);
console.log(`DIRECTORY: ${directory}`);

// Apply remote branch to the workspace.
pushd(directory);
butTestingIn(".", "-j", "stack-branches", "-u", "-b", branch);
popd();
