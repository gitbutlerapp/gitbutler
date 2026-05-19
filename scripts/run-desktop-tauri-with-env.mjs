import { spawn } from "node:child_process";

import { mergeDesktopEnv } from "./desktop-env.mjs";

/** @type {string | undefined} */
const mode = process.argv[2];
/** @type {string | undefined} */
const command = process.argv[3];
const commandArgs = process.argv.slice(4);

if (!mode || !command) {
	console.error("Usage: node scripts/run-desktop-tauri-with-env.mjs <mode> <command> [args...]");
	process.exit(1);
}

const repoRoot = process.cwd();
const mergedEnv = mergeDesktopEnv(mode, { repoRoot });

const child = spawn(command, commandArgs, {
	cwd: repoRoot,
	env: mergedEnv,
	stdio: "inherit",
	shell: process.platform === "win32",
});

child.on("exit", (code, signal) => {
	if (signal) {
		process.kill(process.pid, signal);
		return;
	}
	process.exit(code ?? 0);
});

child.on("error", (error) => {
	console.error(error);
	process.exit(1);
});
