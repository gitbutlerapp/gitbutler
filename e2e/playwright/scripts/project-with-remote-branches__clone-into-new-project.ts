import { logEnv, args, pushd, popd, git, gitOutputIn, butTestingIn } from "./lib.ts";

logEnv();

const projectPath = args[0];
console.log(`PROJECT PATH: ${projectPath}`);

// Clone the remote into a folder and add the project to the application.
git("clone", "remote-project", projectPath);
pushd(projectPath);
git("checkout", "master");
const upstream = gitOutputIn(".", "rev-parse", "--symbolic-full-name", "@{u}");
butTestingIn(".", "add-project", "--switch-to-workspace", upstream);
popd();
