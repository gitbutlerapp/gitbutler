import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

export async function runGit(cwd: string, args: Array<string>): Promise<string> {
	const { stdout } = await execFileAsync("git", args, {
		cwd,
		maxBuffer: 10 * 1024 * 1024,
	});
	return stdout.trim();
}

export async function initializeGitRepository(repositoryPath: string): Promise<void> {
	await fs.mkdir(repositoryPath, { recursive: true });
	await runGit(repositoryPath, ["init"]);
	await runGit(repositoryPath, ["config", "user.name", "How E2E"]);
	await runGit(repositoryPath, ["config", "user.email", "how-e2e@example.com"]);
}

export async function checkpointCommitCount(repositoryPath: string): Promise<number> {
	return (await checkpointCommitIds(repositoryPath)).length;
}

export async function checkpointCommitIds(repositoryPath: string): Promise<Array<string>> {
	const output = await runGit(repositoryPath, ["log", "--format=%H", "--grep=^Checkpoint:"]).catch(
		() => "",
	);
	return output
		.split("\n")
		.map((line) => line.trim())
		.filter(Boolean);
}

export function pathTitle(filePath: string): string {
	return path.basename(filePath);
}
