/** Contains the changes that are in the worktree */
export type WorktreeChanges = {
	/** Changes that could be committed. */
	readonly changes: TreeChange[];
	/**
	 * Changes that were in the index that we can't handle.
	 * The user can see them and interact with them to clear them out before a commit can be made.
	 */
	readonly ignoredChanges: IgnoredChange[];
};
/**
 * An entry in the worktree that changed and thus is eligible to being committed.
 * It either lives (or lived) in the in `.git/index`, or in the `worktree`.
 */

export type TreeChange = {
	/** The *relative* path in the worktree where the entry can be found.*/
	readonly path: string;
	/**
	 * Something silently carried back and forth between the frontend and the backend.
	 * This is neccessary because the path string conversion is lossy.
	 */
	readonly pathBytes: number[];
	/** Previous path bytes if the change is a rename. */
	readonly previousPathBytes: number[];
	/** The specific information about this change.*/
	readonly status: Status;
};

export type Flags =
	| 'ExecutableBitAdded'
	| 'ExecutableBitRemoved'
	| 'TypeChangeFileToLink'
	| 'TypeChangeLinkToFile'
	| 'TypeChange';

export type Status =
	| { readonly type: 'Addition'; readonly subject: Addition }
	| { readonly type: 'Deletion'; readonly subject: Deletion }
	| { readonly type: 'Modification'; readonly subject: Modification }
	| { readonly type: 'Rename'; readonly subject: Rename };
/** Something was added or scheduled to be added.*/

export type Addition = {
	/** @private */
	readonly state: ChangeState;
	readonly isUntracked: boolean;
};
/** Something was deleted.*/

export type Deletion = {
	/** @private */
	readonly previousState: ChangeState;
};
/** A tracked entry was modified, i.e. content change, type change (eg. it is now a symlink), executable bit change.*/

export type Modification = {
	/** @private */
	readonly previousState: ChangeState;
	readonly state: ChangeState;
	readonly flags: Flags | null;
};
/**
 * An entry was renamed from `previous_path` to its current location.
 * Note that this may include a content change, as well as a change of the executable bit.
 */
export type Rename = {
	readonly previousPath: string;
	/** @private */
	readonly previousState: ChangeState;
	/** @private */
	readonly state: ChangeState;
	readonly flags: Flags | null;
};

/**
 * Something that fully identifies the state of a [`TreeChange`] in the backend.
 * The fontend does not need to interact with this, but when requesting the UniDiff of a TreeChange,
 * this information allows for efficient retrieval of the content.
 */
type ChangeState = {
	readonly id: string;
	readonly kind: string;
};

/** A way to indicate that a path in the index isn't suitable for committing and needs to be dealt with.*/
export type IgnoredChange = {
	/** The worktree-relative path to the change.*/
	readonly path: string;
	/** The reason for the change being ignored.*/
	readonly status: IgnoredChangeStatus;
};

/** The status we can't handle.*/
export type IgnoredChangeStatus =
	/** A conflicting entry in the index. The worktree state of the entry is unclear.*/
	| 'Conflict'
	/** A change in the `.git/index` that was overruled by a change to the same path in the *worktree*.*/
	| 'TreeIndex';
