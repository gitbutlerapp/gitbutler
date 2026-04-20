import { logEnv, args, appendLine, pushd, popd, git } from "./lib.ts";

logEnv();

const commitMessage = args[0];
const filePath = args[1];
const fileContent = args[2];

console.log(`COMMIT MESSAGE: ${commitMessage}`);
console.log(`FILE PATH: ${filePath}`);
console.log(`FILE CONTENT: ${fileContent}`);

// Create a new branch in the remote project, add a file, and commit it.
pushd("remote-project");
git("checkout", "master");
appendLine(filePath, fileContent);
git("add", filePath);
git("commit", "-m", commitMessage);
popd();
