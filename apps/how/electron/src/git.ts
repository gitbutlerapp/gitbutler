import {
	defaultProjectSettings,
	normalizeCheckpointDebounceMsWithFallback,
	normalizeCodingAgent,
	normalizeFetchIntervalMsWithFallback,
	normalizeProjectSettings,
	type ProjectSettings,
} from "./settings.js";
import {
	howCreateCheckpoint,
	howCreateBookmark,
	howCreateBookmarkFromCommit,
	howDeleteBookmark,
	howHasProjectChanges,
	howListBookmarks,
	howListCheckpoints,
	howOpenProject,
	howReadProjectSettings,
	howRenameBookmark,
	howRestoreCheckpoint,
	howStagedDiffForCheckpointSummary,
	howStartProject,
	howSwitchBookmark,
	howUpdateCheckpointMessageByChangeId,
	howUpdateBookmark,
	howWriteProjectSettings,
	type HowBookmark,
	type HowBookmarkKind,
	type HowCreateCheckpointResult,
	type HowProject,
	type HowUpdateCheckpointMessageResult,
} from "@gitbutler/but-sdk";
import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
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

async function runGit(
	args: Array<string>,
	cwd: string,
	options: { env?: NodeJS.ProcessEnv } = {},
): Promise<string> {
	try {
		const { stdout } = await execFileAsync("git", args, {
			cwd,
			maxBuffer: 10 * 1024 * 1024,
			env: options.env ? { ...process.env, ...options.env } : process.env,
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
	changeId: string | null;
	title: string;
	createdAt: number;
};

export type CreatedCheckpoint = {
	commitId: string;
	changeId: string;
};

export type GitBookmark = HowBookmark;
export type GitBookmarkKind = HowBookmarkKind;

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

export async function listProjectBookmarks(projectId: string): Promise<Array<GitBookmark>> {
	return await howListBookmarks(projectId);
}

export async function createProjectBookmark(
	projectId: string,
	name: string,
	kind: GitBookmarkKind,
): Promise<GitBookmark> {
	return await howCreateBookmark(projectId, name, kind);
}

export async function createProjectBookmarkFromCommit(
	projectId: string,
	name: string,
	kind: GitBookmarkKind,
	commitId: string,
): Promise<GitBookmark> {
	return await howCreateBookmarkFromCommit(projectId, name, kind, commitId);
}

export async function switchProjectBookmark(projectId: string, bookmarkId: string): Promise<void> {
	await howSwitchBookmark(projectId, bookmarkId);
}

export async function updateProjectBookmark(
	projectId: string,
	bookmarkId: string,
): Promise<GitBookmark> {
	return await howUpdateBookmark(projectId, bookmarkId);
}

export async function renameProjectBookmark(
	projectId: string,
	bookmarkId: string,
	name: string,
): Promise<GitBookmark> {
	return await howRenameBookmark(projectId, bookmarkId, name);
}

export async function deleteProjectBookmark(projectId: string, bookmarkId: string): Promise<void> {
	await howDeleteBookmark(projectId, bookmarkId);
}

export async function createCheckpointCommit(
	projectId: string,
	message: string | (() => Promise<string>),
): Promise<CreatedCheckpoint | null> {
	const resolvedMessage = typeof message === "string" ? message : await message();
	return createdCheckpointFromSdk(await howCreateCheckpoint(projectId, resolvedMessage));
}

function createdCheckpointFromSdk(result: HowCreateCheckpointResult): CreatedCheckpoint | null {
	if (result.type === "unchanged") return null;
	return {
		commitId: result.commit_id,
		changeId: result.change_id,
	};
}

export async function updateCheckpointMessageByChangeId(
	projectId: string,
	changeId: string,
	message: string,
): Promise<HowUpdateCheckpointMessageResult> {
	return await howUpdateCheckpointMessageByChangeId(projectId, changeId, message);
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
	worktreePath: string,
	fallback: ProjectSettings = defaultProjectSettings,
): Promise<ProjectSettings> {
	const settings = await howReadProjectSettings(projectId, fallback);
	const rawFetchIntervalMs = await runGit(
		["config", "--local", "--get", "how.fetchIntervalMs"],
		worktreePath,
	).catch(() => String(fallback.fetchIntervalMs));
	return {
		checkpointDebounceMs:
			settings.checkpointDebounceMs === fallback.checkpointDebounceMs
				? fallback.checkpointDebounceMs
				: normalizeCheckpointDebounceMsWithFallback(
						settings.checkpointDebounceMs,
						fallback.checkpointDebounceMs,
					),
		codingAgent: normalizeCodingAgent(settings.codingAgent),
		fetchIntervalMs: normalizeFetchIntervalMsWithFallback(
			rawFetchIntervalMs,
			fallback.fetchIntervalMs,
		),
	};
}

export async function writeProjectSettings(
	projectId: string,
	worktreePath: string,
	settings: ProjectSettings,
): Promise<void> {
	const normalized = normalizeProjectSettings(settings);
	await howWriteProjectSettings(projectId, normalized);
	await runGit(
		["config", "--local", "how.fetchIntervalMs", String(normalized.fetchIntervalMs)],
		worktreePath,
	);
}

export async function checkpointDiffForSummary(projectId: string): Promise<{
	diff: string;
	originalByteCount: number;
}> {
	return await howStagedDiffForCheckpointSummary(projectId);
}

export async function checkpointDiffForCommit(
	worktreePath: string,
	commitId: string,
): Promise<{
	diff: string;
	originalByteCount: number;
}> {
	const diff = await runGit(
		["show", "--format=", "--stat", "--patch", "--no-ext-diff", commitId],
		worktreePath,
	);
	return {
		diff,
		originalByteCount: Buffer.byteLength(diff, "utf8"),
	};
}

export type PublishMode = "direct" | "review";

export type DirectPublishResult =
	| {
			type: "published";
	  }
	| {
			type: "needsDestination";
	  };

export type SharedProjectStatus =
	| {
			state: "unknown";
			lastCheckedAt: number | null;
			message: string | null;
	  }
	| {
			state: "current";
			lastCheckedAt: number;
			message: string | null;
	  }
	| {
			state: "updateAvailable";
			lastCheckedAt: number;
			message: string | null;
	  }
	| {
			state: "couldNotCheck";
			lastCheckedAt: number | null;
			message: string | null;
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

export function gitErrorDetails(error: unknown): unknown {
	if (error instanceof DirectPublishError)
		return {
			name: error.name,
			kind: error.kind,
			message: error.message,
			cause: gitErrorDetails(error.cause),
		};
	if (error instanceof GitCommandError)
		return {
			name: error.name,
			message: error.message,
			args: error.args,
			stdout: error.stdout,
			stderr: error.stderr,
		};
	if (error instanceof Error)
		return {
			name: error.name,
			message: error.message,
			stack: error.stack,
		};
	return error;
}

export async function readPublishMode(worktreePath: string): Promise<PublishMode | null> {
	const value = await runGit(["config", "--local", "--get", "how.publishMode"], worktreePath).catch(
		() => null,
	);
	return value === "direct" || value === "review" ? value : null;
}

export async function writePublishMode(worktreePath: string, mode: PublishMode): Promise<void> {
	await runGit(["config", "--local", "how.publishMode", mode], worktreePath);
}

export async function publishDirect(
	worktreePath: string,
	options: { destinationUrl?: string; githubToken?: string | null } = {},
): Promise<DirectPublishResult> {
	const branchName = await currentBranchName(worktreePath);
	if (!branchName) {
		throw new DirectPublishError(
			"missingBranch",
			"How could not find the current project version.",
		);
	}

	const upstream = await currentBranchUpstream(worktreePath);
	const gitEnv = options.githubToken ? await gitAskpassEnv(options.githubToken) : undefined;
	try {
		if (upstream) {
			await runGit(["push"], worktreePath, { env: gitEnv });
			return { type: "published" };
		}

		const remotes = await repositoryRemotes(worktreePath);
		if (remotes.length > 0) {
			await pushAndTrack(worktreePath, preferredRemote(remotes), branchName, gitEnv);
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
		await pushAndTrackWithRetry(worktreePath, "origin", branchName, gitEnv);
		return { type: "published" };
	} catch (error) {
		if (error instanceof DirectPublishError) throw error;
		if (isRejectedPush(error))
			throw new DirectPublishError(
				"rejected",
				"The shared project has changes How cannot publish over yet.",
				error,
			);
		throw new DirectPublishError("failed", "How could not publish to the shared project.", error);
	}
}

export async function refreshSharedProject(
	worktreePath: string,
	options: { fetch?: boolean } = {},
): Promise<SharedProjectStatus> {
	const upstream = await currentBranchUpstream(worktreePath);
	if (!upstream) return { state: "unknown", lastCheckedAt: null, message: null };

	try {
		if (options.fetch ?? true) await fetchUpstream(worktreePath, upstream);
		const upstreamTip = await runGit(["rev-parse", "--verify", "@{u}"], worktreePath);
		const mergeBase = await runGit(["merge-base", "HEAD", "@{u}"], worktreePath);
		const checkedAt = Date.now();
		if (mergeBase === upstreamTip) {
			return { state: "current", lastCheckedAt: checkedAt, message: null };
		}
		return {
			state: "updateAvailable",
			lastCheckedAt: checkedAt,
			message: "Update available",
		};
	} catch (error) {
		throw new DirectPublishError(
			"failed",
			"How could not check for shared updates.",
			error,
		);
	}
}

export async function filterUnpublishedCommits(
	worktreePath: string,
	commits: Array<GitCommit>,
): Promise<Array<GitCommit>> {
	const upstream = await currentBranchUpstream(worktreePath);
	if (!upstream) return commits;
	const upstreamExists = await runGit(["rev-parse", "--verify", "@{u}"], worktreePath)
		.then(() => true)
		.catch(() => false);
	if (!upstreamExists) return commits;

	const unpublished: Array<GitCommit> = [];
	for (const commit of commits) {
		const published = await commitIsAncestor(worktreePath, commit.id, "@{u}");
		if (!published) unpublished.push(commit);
	}
	return unpublished;
}

export async function hasAnyRemote(worktreePath: string): Promise<boolean> {
	return (await repositoryRemotes(worktreePath)).length > 0;
}

export async function hasGithubDestination(worktreePath: string): Promise<boolean> {
	const upstream = await currentBranchUpstream(worktreePath);
	if (upstream) {
		const remote = upstream.split("/")[0];
		if (remote && (await remoteIsGithub(worktreePath, remote))) return true;
	}
	const remotes = await repositoryRemotes(worktreePath);
	for (const remote of remotes) {
		if (await remoteIsGithub(worktreePath, remote)) return true;
	}
	return false;
}

export function sanitizedRepositoryName(projectTitle: string): string {
	const sanitized = projectTitle
		.toLowerCase()
		.replace(/[^a-z0-9._-]+/g, "-")
		.replace(/^-+|-+$/g, "")
		.slice(0, 80);
	return sanitized || "how-project";
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

async function fetchUpstream(worktreePath: string, upstream: string): Promise<void> {
	const [remote, ...branchParts] = upstream.split("/");
	const branch = branchParts.join("/");
	if (!remote || !branch) {
		await runGit(["fetch", "--quiet"], worktreePath);
		return;
	}
	await runGit(
		["fetch", "--quiet", remote, `+refs/heads/${branch}:refs/remotes/${remote}/${branch}`],
		worktreePath,
	);
}

async function commitIsAncestor(
	worktreePath: string,
	commit: string,
	ancestorOf: string,
): Promise<boolean> {
	try {
		await runGit(["merge-base", "--is-ancestor", commit, ancestorOf], worktreePath);
		return true;
	} catch {
		return false;
	}
}

async function repositoryRemotes(worktreePath: string): Promise<Array<string>> {
	const output = await runGit(["remote"], worktreePath).catch(() => "");
	return output
		.split("\n")
		.map((remote) => remote.trim())
		.filter(Boolean);
}

async function remoteIsGithub(worktreePath: string, remote: string): Promise<boolean> {
	const url = await runGit(["remote", "get-url", remote], worktreePath).catch(() => "");
	return isGithubRemoteUrl(url);
}

function isGithubRemoteUrl(url: string): boolean {
	return (
		url.startsWith("https://github.com/") ||
		url.startsWith("git@github.com:") ||
		url.includes("@github.com/")
	);
}

function preferredRemote(remotes: Array<string>): string {
	return remotes.includes("origin") ? "origin" : (remotes[0] ?? "origin");
}

async function pushAndTrack(
	worktreePath: string,
	remote: string,
	branchName: string,
	env?: NodeJS.ProcessEnv,
): Promise<void> {
	await runGit(["push", "-u", remote, `HEAD:${branchName}`], worktreePath, { env });
}

async function pushAndTrackWithRetry(
	worktreePath: string,
	remote: string,
	branchName: string,
	env?: NodeJS.ProcessEnv,
): Promise<void> {
	let lastError: unknown;
	for (let attempt = 0; attempt < 4; attempt += 1) {
		try {
			await pushAndTrack(worktreePath, remote, branchName, env);
			return;
		} catch (error) {
			lastError = error;
			if (!looksTemporarilyUnavailable(error)) throw error;
			await new Promise((resolve) => setTimeout(resolve, 750 * (attempt + 1)));
		}
	}
	throw lastError;
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

function looksTemporarilyUnavailable(error: unknown): boolean {
	if (!(error instanceof GitCommandError)) return false;
	const output = `${error.stdout}\n${error.stderr}`.toLowerCase();
	return output.includes("repository not found") || output.includes("not found");
}

async function gitAskpassEnv(token: string): Promise<NodeJS.ProcessEnv> {
	const scriptPath = path.join(os.tmpdir(), "how-git-askpass.sh");
	await fs.writeFile(
		scriptPath,
		[
			"#!/bin/sh",
			'case "$1" in',
			"  *sername*|*Username*) printf '%s' 'x-access-token' ;;",
			"  *) printf '%s' \"$HOW_GIT_ASKPASS_TOKEN\" ;;",
			"esac",
		].join("\n"),
		{ mode: 0o700 },
	);
	return {
		GIT_ASKPASS: scriptPath,
		GIT_TERMINAL_PROMPT: "0",
		HOW_GIT_ASKPASS_TOKEN: token,
	};
}
