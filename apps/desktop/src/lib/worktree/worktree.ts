import { listen, invoke } from '$lib/backend/ipc';

/** Gets the current status of the worktree */
export async function worktree_changes(projectId: string): Promise<WorktreeChanges> {
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

/** Contains the changes that are in the worktree */
export class WorktreeChanges {
	/** Changes that could be committed. */
	changes!: [TreeChange];
	/**
	Changes that were in the index that we can't handle.
	The user can see them and interact with them to clear them out before a commit can be made.
	*/
	ignoredChanges!: [IgnoredChange];
}

/**
An entry in the worktree that changed and thus is eligible to being committed.
It either lives (or lived) in the in `.git/index`, or in the `worktree`.
*/
export class TreeChange {
	/** The *relative* path in the worktree where the entry can be found.*/
	path!: string;
	/** The specific information about this change.*/
	status!: Status;
}

export type Flags =
	| 'ExecutableBitAdded'
	| 'ExecutableBitRemoved'
	| 'TypeChangeFileToLink'
	| 'TypeChangeLinkToFile'
	| 'TypeChange';

export type Status =
	/** Something was added or scheduled to be added.*/
	| { type: 'Addition'; subject: { state: ChangeState; isUntracked: boolean } }
	/** Something was deleted.*/
	| { type: 'Deletion'; subject: { previousState: ChangeState } }
	/** A tracked entry was modified, i.e. content change, type change (eg. it is now a symlink), executable bit change.*/
	| {
			type: 'Modification';
			subject: { previousState: ChangeState; state: ChangeState; flags: Flags | null };
	  }
	/**
	An entry was renamed from `previous_path` to its current location.
	Note that this may include a content change, as well as a change of the executable bit.
	*/
	| {
			type: 'Rename';
			subject: {
				previousPath: string;
				previousState: ChangeState;
				state: ChangeState;
				flags: Flags | null;
			};
	  };

/**
Something that fully identifies the state of a [`TreeChange`] in the backend.
The fontend does not need to interact with this, but when requesting the UniDiff of a TreeChange,
this information allows for efficient retrieval of the content.
*/
class ChangeState {
	id!: string;
	kind!: string;
}

/** A way to indicate that a path in the index isn't suitable for committing and needs to be dealt with.*/
export class IgnoredChange {
	/** The worktree-relative path to the change.*/
	path!: string;
	/** The reason for the change being ignored.*/
	status!: IgnoredChangeStatus;
}

/** The status we can't handle.*/
export type IgnoredChangeStatus =
	/** A conflicting entry in the index. The worktree state of the entry is unclear.*/
	| 'Conflict'
	/** A change in the `.git/index` that was overruled by a change to the same path in the *worktree*.*/
	| 'TreeIndex';
