import { getDependencyCommitIds, getHunkDependencyDiffsByPath } from "#ui/hunk.ts";
import { buildFileTreeRows, type FileDisplayMode, type FileTreeRow } from "./file-tree.ts";
import type { TreeChange, WorktreeChanges } from "@gitbutler/but-sdk";

type ChangeFileRowItem = {
	change: TreeChange;
	dependencyCommitIds: Array<string>;
	path: string;
};

export const changeFileRowItem = ({
	change,
	dependencyCommitIds,
	path,
}: ChangeFileRowItem): FileRowItem => ({
	_tag: "Change",
	change,
	dependencyCommitIds,
	path,
});

type ConflictFileRowItem = {
	path: string;
};

export const conflictFileRowItem = ({ path }: ConflictFileRowItem): FileRowItem => ({
	_tag: "Conflict",
	path,
});

export const getChangesFileRowItems = (worktreeChanges: WorktreeChanges): Array<FileRowItem> => {
	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);

	// Conflicted files are kept out of `changes` until resolved.
	const conflicts = worktreeChanges.ignoredChanges.flatMap((change) =>
		change.status === "Conflict" ? [conflictFileRowItem({ path: change.path })] : [],
	);

	const changes = worktreeChanges.changes.map((change) => {
		const hunkDependencyDiffs = hunkDependencyDiffsByPath.get(change.path);
		const dependencyCommitIds = hunkDependencyDiffs
			? getDependencyCommitIds({ hunkDependencyDiffs })
			: [];

		return changeFileRowItem({
			change,
			dependencyCommitIds,
			path: change.path,
		});
	});

	return [...conflicts, ...changes];
};

/**
 * The rows of the uncommitted files list. The page's address space and the
 * sidebar's list are both built from this, so they always agree on which rows exist.
 */
export const buildUncommittedFileRows = ({
	worktreeChanges,
	filter,
	mode,
	collapsedDirectories,
}: {
	worktreeChanges: WorktreeChanges | undefined;
	filter: string | null;
	mode: FileDisplayMode;
	collapsedDirectories: Record<string, true>;
}): Array<FileTreeRow<FileRowItem>> =>
	buildFileTreeRows({
		items: (worktreeChanges ? getChangesFileRowItems(worktreeChanges) : []).filter((item) =>
			pathMatchesFilter(item.path, filter),
		),
		mode,
		collapsedDirectories,
	});

/**
 * Case-insensitive substring match over the whole path, so a directory narrows
 * the list just as a file name does. A blank query — including a filter that is
 * open but not yet typed into — matches everything.
 */
export const pathMatchesFilter = (path: string, filter: string | null): boolean => {
	const query = filter?.trim().toLowerCase() ?? "";
	return query === "" || path.toLowerCase().includes(query);
};

export type FileRowItem =
	| ({ _tag: "Change" } & ChangeFileRowItem)
	| ({ _tag: "Conflict" } & ConflictFileRowItem);
