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
