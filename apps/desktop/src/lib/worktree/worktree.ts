import { listen, invoke } from '$lib/backend/ipc';

/** Gets the current status of the worktree */
export async function worktreeChanges(projectId: string): Promise<WorktreeChanges> {
	return await invoke<WorktreeChanges>('worktree_changes', { projectId });
}

/** Subscibes for worktree_changes updates */
export function subscribe<WorktreeChanges>(
	projectId: string,
	callback: (changes: WorktreeChanges) => void
) {
	return listen<WorktreeChanges>(`project://${projectId}/worktree_changes`, (event) =>
		callback(event.payload)
	);
}

/**
 * Gets the unified diff for a given TreeChange.
 * This probably does not belong in a package called "worktree" since this also operates on commit-to-commit changes and not only worktree changes
 */
export async function treeChangeDiffs(projectId: string, changes: TreeChange[]) {
	return await invoke<UnifiedDiff[]>('tree_change_diffs', { projectId, changes });
}

/**
 * A patch in unified diff format to show how a resource changed or now looks like (in case it was newly added),
 * or how it previously looked like in case of a deletion.
 */
export type UnifiedDiff =
	| { type: 'Binary' } // A binary file that can't be diffed.
	| { type: 'TooLarge'; subject: TooLarge }
	| { type: 'Patch'; subject: Patch };

/** The file was too large and couldn't be diffed. */
export type TooLarge = {
	/** The size of the file on disk that made it too large. */
	sizeInBytes: number;
};

/**
 * A patch that if applied to the previous state of the resource would yield the current state.
 * Includes all non-overlapping hunks, including their context lines.
 */
export type Patch = {
	/** All non-overlapping hunks, including their context lines. */
	hunks: DiffHunk[];
};

/** A hunk as used in UnifiedDiff. */
export type DiffHunk = {
	/** The 1-based line number at which the previous version of the file started.*/
	oldStart: number;
	/** The non-zero amount of lines included in the previous version of the file.*/
	oldLines: number;
	/** The 1-based line number at which the new version of the file started.*/
	newStart: number;
	/** The non-zero amount of lines included in the new version of the file.*/
	newLines: number;
	/**
	 * A unified-diff formatted patch like:
	 *
	 * ```diff
	 * @@ -1,6 +1,8 @@
	 * This is the first line of the original text.
	 * -Line to be removed.
	 * +Line that has been replaced.
	 * This is another line in the file.
	 * +This is a new line added at the end.
	 * ```
	 *
	 * The line separator is the one used in the original file and may be `LF` or `CRLF`.
	 * Note that the file-portion of the header isn't used here.
	 */
	diff: string;
};

/** Contains the changes that are in the worktree */
export type WorktreeChanges = {
	/** Changes that could be committed. */
	changes: TreeChange[];
	/**
	 * Changes that were in the index that we can't handle.
	 * The user can see them and interact with them to clear them out before a commit can be made.
	 */
	ignoredChanges: IgnoredChange[];
};

/**
 * An entry in the worktree that changed and thus is eligible to being committed.
 * It either lives (or lived) in the in `.git/index`, or in the `worktree`.
 */
export type TreeChange = {
	/** The *relative* path in the worktree where the entry can be found.*/
	path: string;
	/** The specific information about this change.*/
	status: Status;
};

export type Flags =
	| 'ExecutableBitAdded'
	| 'ExecutableBitRemoved'
	| 'TypeChangeFileToLink'
	| 'TypeChangeLinkToFile'
	| 'TypeChange';

export type Status =
	| { type: 'Addition'; subject: Addition }
	| { type: 'Deletion'; subject: Deletion }
	| { type: 'Modification'; subject: Modification }
	| { type: 'Rename'; subject: Rename };

/** Something was added or scheduled to be added.*/
export type Addition = {
	state: ChangeState;
	isUntracked: boolean;
};

/** Something was deleted.*/
export type Deletion = {
	previousState: ChangeState;
};

/** A tracked entry was modified, i.e. content change, type change (eg. it is now a symlink), executable bit change.*/
export type Modification = {
	previousState: ChangeState;
	state: ChangeState;
	flags: Flags | null;
};

/**
 * An entry was renamed from `previous_path` to its current location.
 * Note that this may include a content change, as well as a change of the executable bit.
 */
export type Rename = {
	previousPath: string;
	previousState: ChangeState;
	state: ChangeState;
	flags: Flags | null;
};

/**
 * Something that fully identifies the state of a [`TreeChange`] in the backend.
 * The fontend does not need to interact with this, but when requesting the UniDiff of a TreeChange,
 * this information allows for efficient retrieval of the content.
 */
type ChangeState = {
	id: string;
	kind: string;
};

/** A way to indicate that a path in the index isn't suitable for committing and needs to be dealt with.*/
export type IgnoredChange = {
	/** The worktree-relative path to the change.*/
	path: string;
	/** The reason for the change being ignored.*/
	status: IgnoredChangeStatus;
};

/** The status we can't handle.*/
export type IgnoredChangeStatus =
	/** A conflicting entry in the index. The worktree state of the entry is unclear.*/
	| 'Conflict'
	/** A change in the `.git/index` that was overruled by a change to the same path in the *worktree*.*/
	| 'TreeIndex';
