import { BUT } from "./env.ts";
import { execFile } from "node:child_process";

export interface ButResult {
	stdout: string;
	stderr: string;
	exitCode: number;
}

/**
 * Run `but` with the given arguments in the given working directory.
 * Returns the full result including stdout, stderr, and exit code.
 */
export function but(
	args: string[],
	cwd: string,
	env?: Record<string, string>,
): Promise<ButResult> {
	return new Promise((resolve, reject) => {
		const child = execFile(
			BUT,
			args,
			{
				cwd,
				env: {
					...process.env,
					// Ensure non-interactive mode for all commands.
					TERM: "dumb",
					CI: "true",
					...env,
				},
				maxBuffer: 10 * 1024 * 1024,
				timeout: 60_000,
			},
			(error, stdout, stderr) => {
				const exitCode = error?.code
					? typeof error.code === "number"
						? error.code
						: 1
					: 0;
				resolve({ stdout, stderr, exitCode });
			},
		);

		child.on("error", reject);
	});
}

/**
 * Run `but` and assert it succeeds (exit code 0). Returns stdout.
 * Throws with stderr on failure.
 */
export async function butOk(
	args: string[],
	cwd: string,
	env?: Record<string, string>,
): Promise<string> {
	const result = await but(args, cwd, env);
	if (result.exitCode !== 0) {
		throw new Error(
			`but ${args.join(" ")} failed (exit ${result.exitCode}):\n${result.stderr}\n${result.stdout}`,
		);
	}
	return result.stdout;
}

/**
 * Run `but` with --json and parse the result.
 */
export async function butJson<T = unknown>(
	args: string[],
	cwd: string,
	env?: Record<string, string>,
): Promise<T> {
	const stdout = await butOk(["--json", ...args], cwd, env);
	return JSON.parse(stdout) as T;
}
