import { getDependencyCommitIds, getHunkDependencyDiffsByPath } from "#ui/hunk.ts";
import type { TreeChange, WorktreeChanges } from "@gitbutler/but-sdk";
import type { HeadInfoIndex } from "#ui/api/ref-info.ts";

type ChangeFileRowItem = {
	change: TreeChange;
	dependencyChangeIds: Array<string>;
	path: string;
};

export const changeFileRowItem = ({
	change,
	dependencyChangeIds,
	path,
}: ChangeFileRowItem): FileRowItem => ({
	_tag: "Change",
	change,
	dependencyChangeIds,
	path,
});

type ConflictFileRowItem = {
	path: string;
};

export const conflictFileRowItem = ({ path }: ConflictFileRowItem): FileRowItem => ({
	_tag: "Conflict",
	path,
});

export const getChangesFileRowItems = (
	worktreeChanges: WorktreeChanges,
	headInfoIndex: HeadInfoIndex,
): Array<FileRowItem> => {
	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);

	return worktreeChanges.changes.map((change) => {
		const hunkDependencyDiffs = hunkDependencyDiffsByPath.get(change.path);
		const dependencyCommitIds = hunkDependencyDiffs
			? getDependencyCommitIds({ hunkDependencyDiffs })
			: [];
		const dependencyChangeIds = dependencyCommitIds.flatMap((commitId) => {
			const changeId = headInfoIndex.commitContextById(commitId)?.commit.changeId;
			return changeId !== undefined ? [changeId] : [];
		});

		return changeFileRowItem({
			change,
			dependencyChangeIds,
			path: change.path,
		});
	});
};

export type FileRowItem =
	| ({ _tag: "Change" } & ChangeFileRowItem)
	| ({ _tag: "Conflict" } & ConflictFileRowItem);
