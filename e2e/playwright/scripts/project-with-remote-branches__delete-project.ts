import { logEnv, args, butTesting } from "./lib.ts";

logEnv();

const projectName = args[0];
console.log(`PROJECT NAME: ${projectName}`);

// Remove the project from the application.
butTesting("remove-project", projectName);
