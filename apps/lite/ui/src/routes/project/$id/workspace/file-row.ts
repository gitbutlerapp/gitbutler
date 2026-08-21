import { getDependencyCommitIds, getHunkDependencyDiffsByPath } from "#ui/hunk.ts";
import { compareFilePaths } from "#ui/file-order.ts";
import { buildFileTreeRows, type FileDisplayMode, type FileTreeRow } from "./file-tree.ts";
import type { TreeChange, WorktreeChanges } from "@gitbutler/but-sdk";

type ChangeFileRowItem = {
	change: TreeChange;
	dependencyCommitIds: Array<string>;
	path: string;
	modifiedAtMs?: number | null;
};

export const changeFileRowItem = ({
	change,
	dependencyCommitIds,
	path,
	modifiedAtMs = null,
}: ChangeFileRowItem): FileRowItem => ({
	_tag: "Change",
	change,
	dependencyCommitIds,
	path,
	modifiedAtMs,
});

type ConflictFileRowItem = {
	path: string;
	modifiedAtMs?: number | null;
};

export const conflictFileRowItem = ({
	path,
	modifiedAtMs = null,
}: ConflictFileRowItem): FileRowItem => ({
	_tag: "Conflict",
	path,
	modifiedAtMs,
});

export const getChangesFileRowItems = (worktreeChanges: WorktreeChanges): Array<FileRowItem> => {
	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);

	// Conflicted files are kept out of `changes` until resolved, but they still
	// sit on disk, so they carry a modification time like any other row.
	const conflicts = worktreeChanges.ignoredChanges.flatMap((change) =>
		change.status === "Conflict"
			? [
					conflictFileRowItem({
						path: change.path,
						modifiedAtMs: worktreeChanges.modificationTimes[change.path] ?? null,
					}),
				]
			: [],
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
			modifiedAtMs: worktreeChanges.modificationTimes[change.path] ?? null,
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
	recentFirst,
}: {
	worktreeChanges: WorktreeChanges | undefined;
	filter: string | null;
	mode: FileDisplayMode;
	collapsedDirectories: Record<string, true>;
	recentFirst: boolean;
}): Array<FileTreeRow<FileRowItem>> =>
	buildFileTreeRows({
		items: (worktreeChanges ? getChangesFileRowItems(worktreeChanges) : []).filter((item) =>
			pathMatchesFilter(item.path, filter),
		),
		mode,
		collapsedDirectories,
		compare: recentFirst ? compareRecentFirst : undefined,
	});

/**
 * Newest first; files with no time (deletions foremost) last, by path. Missing is
 * its own case, not a zero sentinel, so a genuine epoch mtime still sorts as one.
 */
const compareRecentFirst = (a: FileRowItem, b: FileRowItem): number => {
	const aModified = a.modifiedAtMs ?? null;
	const bModified = b.modifiedAtMs ?? null;
	if (aModified === null || bModified === null) {
		if (aModified !== null) return -1;
		if (bModified !== null) return 1;
		return compareFilePaths(a.path, b.path);
	}
	return bModified !== aModified ? bModified - aModified : compareFilePaths(a.path, b.path);
};

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
