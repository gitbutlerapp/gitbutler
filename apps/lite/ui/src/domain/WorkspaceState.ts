import { type WorkspaceState } from "@gitbutler/but-sdk";

export const getConflictedCommitIds = (workspace: WorkspaceState): Set<string> => {
	const previousCommitIdByNextCommitId = new Map(
		Object.entries(workspace.replacedCommits).map(([previousCommitId, nextCommitId]) => [
			nextCommitId,
			previousCommitId,
		]),
	);

	return new Set(
		workspace.headInfo.stacks.flatMap((stack) =>
			stack.segments.flatMap((segment) =>
				segment.commits
					.filter((commit) => commit.hasConflicts)
					.map((commit) => previousCommitIdByNextCommitId.get(commit.id) ?? commit.id),
			),
		),
	);
};
