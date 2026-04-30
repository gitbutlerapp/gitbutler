import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

function parseShellEnvironment(output: string): Record<string, string> {
	const env: Record<string, string> = {};

	for (const line of output.split("\n")) {
		if (!line) continue;
		const separatorIndex = line.indexOf("=");
		if (separatorIndex <= 0) continue;

		const key = line.slice(0, separatorIndex);
		const value = line.slice(separatorIndex + 1);
		env[key] = value;
	}

	return env;
}

export async function inheritLoginShellEnvironmentIfNeeded(): Promise<void> {
	if (process.env.TERM !== undefined) {
		return;
	}

	const shellPath = process.env.SHELL;
	if (!shellPath) {
		return;
	}

	try {
		const { stdout } = await execFileAsync(shellPath, ["-i", "-l", "-c", "env"], {
			timeout: 5_000,
			maxBuffer: 10 * 1024 * 1024,
		});

		const shellEnv = parseShellEnvironment(stdout);
		Object.assign(process.env, shellEnv);
	} catch (error) {
		console.warn("Failed to inherit login-shell environment; continuing with GUI defaults", error);
	}
}
