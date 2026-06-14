import {
	defaultProjectSettings,
	normalizeCheckpointDebounceMsWithFallback,
	normalizeCodingAgent,
	normalizeProjectSettings,
	type ProjectSettings,
} from "./settings.js";
import {
	howCreateCheckpoint,
	howHasProjectChanges,
	howListCheckpoints,
	howOpenProject,
	howReadProjectSettings,
	howRestoreCheckpoint,
	howStagedDiffForCheckpointSummary,
	howStartProject,
	howWriteProjectSettings,
	type HowProject,
} from "@gitbutler/but-sdk";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

export class GitCommandError extends Error {
	constructor(
		message: string,
		readonly args: Array<string>,
		readonly stdout: string,
		readonly stderr: string,
	) {
		super(message);
		this.name = "GitCommandError";
	}
}

async function runGit(args: Array<string>, cwd: string): Promise<string> {
	try {
		const { stdout } = await execFileAsync("git", args, {
			cwd,
			maxBuffer: 10 * 1024 * 1024,
		});
		return stdout.trim();
	} catch (error) {
		const maybeError = error as {
			message?: string;
			stdout?: string | Buffer;
			stderr?: string | Buffer;
		};
		const stdout = Buffer.isBuffer(maybeError.stdout)
			? maybeError.stdout.toString("utf8")
			: (maybeError.stdout ?? "");
		const stderr = Buffer.isBuffer(maybeError.stderr)
			? maybeError.stderr.toString("utf8")
			: (maybeError.stderr ?? "");
		throw new GitCommandError(maybeError.message ?? "Git command failed.", args, stdout, stderr);
	}
}

export type GitCommit = {
	id: string;
	title: string;
	createdAt: number;
};

export type GitRepository = {
	id: string;
	title: string;
	gitDir: string;
	worktreePath: string;
};

export function projectFromSdk(project: HowProject): GitRepository {
	return {
		id: project.id,
		title: project.title,
		gitDir: project.gitDir,
		worktreePath: project.path,
	};
}

export async function discoverRepository(directory: string): Promise<GitRepository> {
	return projectFromSdk(await howOpenProject(directory));
}

export async function ensureGitRepository(directory: string): Promise<GitRepository> {
	return projectFromSdk(await howStartProject(directory));
}

export async function listCheckpointCommits(
	projectId: string,
	limit: number,
): Promise<Array<GitCommit>> {
	return await howListCheckpoints(projectId, limit);
}

export async function createCheckpointCommit(
	projectId: string,
	message: string | (() => Promise<string>),
): Promise<string | null> {
	const resolvedMessage = typeof message === "string" ? message : await message();
	return await howCreateCheckpoint(projectId, resolvedMessage);
}

export async function resetToCommit(
	projectId: string,
	commitId: string,
	options: { discardChanges?: boolean } = {},
): Promise<void> {
	await howRestoreCheckpoint(projectId, commitId, options.discardChanges ?? false);
}

export async function hasWorktreeChanges(projectId: string): Promise<boolean> {
	return await howHasProjectChanges(projectId);
}

export async function readProjectSettings(
	projectId: string,
	fallback: ProjectSettings = defaultProjectSettings,
): Promise<ProjectSettings> {
	const settings = await howReadProjectSettings(projectId, fallback);
	return {
		checkpointDebounceMs:
			settings.checkpointDebounceMs === fallback.checkpointDebounceMs
				? fallback.checkpointDebounceMs
				: normalizeCheckpointDebounceMsWithFallback(
						settings.checkpointDebounceMs,
						fallback.checkpointDebounceMs,
					),
		codingAgent: normalizeCodingAgent(settings.codingAgent),
	};
}

export async function writeProjectSettings(
	projectId: string,
	settings: ProjectSettings,
): Promise<void> {
	await howWriteProjectSettings(projectId, normalizeProjectSettings(settings));
}

export async function checkpointDiffForSummary(projectId: string): Promise<{
	diff: string;
	originalByteCount: number;
}> {
	return await howStagedDiffForCheckpointSummary(projectId);
}

export type PublishMode = "direct";

export type DirectPublishResult =
	| {
			type: "published";
	  }
	| {
			type: "needsDestination";
	  };

export type DirectPublishErrorKind =
	| "addDestinationFailed"
	| "missingBranch"
	| "rejected"
	| "failed";

export class DirectPublishError extends Error {
	constructor(
		readonly kind: DirectPublishErrorKind,
		message: string,
		readonly cause?: unknown,
	) {
		super(message);
		this.name = "DirectPublishError";
	}
}

export async function readPublishMode(worktreePath: string): Promise<PublishMode | null> {
	const value = await runGit(["config", "--local", "--get", "how.publishMode"], worktreePath).catch(
		() => null,
	);
	return value === "direct" ? "direct" : null;
}

export async function writePublishMode(worktreePath: string, mode: PublishMode): Promise<void> {
	await runGit(["config", "--local", "how.publishMode", mode], worktreePath);
}

export async function publishDirect(
	worktreePath: string,
	options: { destinationUrl?: string } = {},
): Promise<DirectPublishResult> {
	const branchName = await currentBranchName(worktreePath);
	if (!branchName) {
		throw new DirectPublishError(
			"missingBranch",
			"How could not find the current project version.",
		);
	}

	const upstream = await currentBranchUpstream(worktreePath);
	try {
		if (upstream) {
			await runGit(["push"], worktreePath);
			return { type: "published" };
		}

		const remotes = await repositoryRemotes(worktreePath);
		if (remotes.length > 0) {
			await pushAndTrack(worktreePath, preferredRemote(remotes), branchName);
			return { type: "published" };
		}

		const destinationUrl = options.destinationUrl?.trim();
		if (!destinationUrl) return { type: "needsDestination" };

		try {
			await runGit(["remote", "add", "origin", destinationUrl], worktreePath);
		} catch (error) {
			throw new DirectPublishError(
				"addDestinationFailed",
				"How could not add that project destination.",
				error,
			);
		}
		await pushAndTrack(worktreePath, "origin", branchName);
		return { type: "published" };
	} catch (error) {
		if (error instanceof DirectPublishError) throw error;
		if (isRejectedPush(error))
			throw new DirectPublishError(
				"rejected",
				"The shared project has changes How cannot publish over yet.",
				error,
			);
		throw new DirectPublishError(
			"failed",
			"How could not publish to the shared project.",
			error,
		);
	}
}

async function currentBranchName(worktreePath: string): Promise<string | null> {
	const branchName = await runGit(["branch", "--show-current"], worktreePath).catch(() => "");
	return branchName.length > 0 ? branchName : null;
}

async function currentBranchUpstream(worktreePath: string): Promise<string | null> {
	const upstream = await runGit(
		["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
		worktreePath,
	).catch(() => "");
	return upstream.length > 0 ? upstream : null;
}

async function repositoryRemotes(worktreePath: string): Promise<Array<string>> {
	const output = await runGit(["remote"], worktreePath).catch(() => "");
	return output
		.split("\n")
		.map((remote) => remote.trim())
		.filter(Boolean);
}

function preferredRemote(remotes: Array<string>): string {
	return remotes.includes("origin") ? "origin" : (remotes[0] ?? "origin");
}

async function pushAndTrack(
	worktreePath: string,
	remote: string,
	branchName: string,
): Promise<void> {
	await runGit(["push", "-u", remote, `HEAD:${branchName}`], worktreePath);
}

function isRejectedPush(error: unknown): boolean {
	if (!(error instanceof GitCommandError)) return false;
	const output = `${error.stdout}\n${error.stderr}`.toLowerCase();
	return (
		output.includes("non-fast-forward") ||
		output.includes("fetch first") ||
		output.includes("failed to push some refs") ||
		output.includes("rejected")
	);
}
