import { logEnv, args, pushd, popd, butTestingIn } from "./lib.ts";

logEnv();

const destination = args[0];
const branchToMove = args[1];
const directory = args[2];

console.log(`BRANCH DESTINATION: ${destination}`);
console.log(`BRANCH TO MOVE: ${branchToMove}`);
console.log(`DIRECTORY: ${directory}`);

// Move branch using but-testing CLI.
pushd(directory);
butTestingIn(".", "-j", "move-branch", destination, branchToMove);
popd();
