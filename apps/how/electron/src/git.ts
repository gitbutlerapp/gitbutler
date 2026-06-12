import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

export type GitCommit = {
	id: string;
	title: string;
	createdAt: number;
};

export function encodeProjectHandle(gitDir: string): string {
	return Buffer.from(gitDir).toString("base64url");
}

export async function runGit(
	args: Array<string>,
	options: { cwd?: string } = {},
): Promise<string> {
	const { stdout } = await execFileAsync("git", args, {
		cwd: options.cwd,
		maxBuffer: 10 * 1024 * 1024,
	});
	return stdout.trim();
}

export async function discoverGitDir(directory: string): Promise<string> {
	const gitDir = await runGit(["rev-parse", "--absolute-git-dir"], { cwd: directory });
	return await fs.realpath(gitDir);
}

export async function ensureGitRepository(directory: string): Promise<string> {
	await fs.mkdir(directory, { recursive: true });
	try {
		return await discoverGitDir(directory);
	} catch {
		await runGit(["init"], { cwd: directory });
		return await discoverGitDir(directory);
	}
}

export async function worktreeFromGitDir(gitDir: string): Promise<string> {
	const worktree = await runGit(["--git-dir", gitDir, "rev-parse", "--show-toplevel"]);
	return await fs.realpath(worktree);
}

export function projectTitleFromPath(worktreePath: string): string {
	const title = path.basename(worktreePath);
	return title.length > 0 ? title : worktreePath;
}

export async function listCheckpointCommits(
	worktreePath: string,
	limit: number,
): Promise<Array<GitCommit>> {
	const output = await runGit(
		[
			"log",
			`--max-count=${limit}`,
			"--format=%H%x1f%ct%x1f%s%x1e",
			"--grep=^Checkpoint:",
			"--fixed-strings",
		],
		{ cwd: worktreePath },
	).catch(() => "");

	return output
		.split("\x1e")
		.map((entry) => entry.trim())
		.filter((entry) => entry.length > 0)
		.map((entry) => {
			const [id, timestamp, title] = entry.split("\x1f");
			if (!id || !timestamp || !title) return null;
			return {
				id,
				title,
				createdAt: Number(timestamp) * 1000,
			};
		})
		.filter((commit): commit is GitCommit => commit !== null);
}

export async function createInitialCheckpointCommit(
	worktreePath: string,
	message: string,
): Promise<string | null> {
	await runGit(["add", "--all"], { cwd: worktreePath });

	const status = await runGit(["status", "--porcelain"], { cwd: worktreePath });
	if (status.length === 0) return null;

	await runGit(["commit", "--message", message], { cwd: worktreePath });
	return await runGit(["rev-parse", "HEAD"], { cwd: worktreePath });
}
