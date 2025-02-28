/**
 * Represents a branch in a `Stack`.
 * It contains commits derived from the local pseudo branch and it's respective remote.
 * This is distinct from a git branch in the sense that it has no local reference - something that we may want to change in the future.
 */
export type StackBranch = {
	/** The name of the branch */
	readonly name: string;
	readonly remoteTrackingBranch: string | null;
	/**
	 * Description of the branch.
	 * Can include arbitrary utf8 data, eg. markdown etc.
	 */
	readonly description: string | null;
	/** The pull(merge) request associated with the branch, or None if no such entity has not been created. */
	readonly prNumber: string | null;
	/** A unique identifier for the GitButler review associated with the branch, if any. */
	readonly reviewId: string | null;
	/**
	 *
	 * Indicates that the branch was previously part of a stack but it has since been integrated.
	 * In other words, the merge base of the stack is now above this branch.
	 * This would occur when the branch has been merged at the remote and the workspace has been updated with that change.
	 * An archived branch will not have any commits associated with it.
	 */
	archived: boolean;
};

/** Commit that is a part of a [`StackBranch`](gitbutler_stack::StackBranch) and, as such, containing state derived in relation to the specific branch.*/
export type Commit = {
	/** The OID of the commit.*/
	readonly id: string;
	/** The parent OIDs of the commit. */
	readonly parentIds: string[];
	/** The message of the commit.*/
	message: string;
	/**
	 * Whether the commit is in a conflicted state.
	 * Conflicted state of a commit is a GitButler concept.
	 * GitButler will perform rebasing/reordering etc without interruptions and flag commits as conflicted if needed.
	 * Conflicts are resolved via the Edit Mode mechanism.
	 */
	readonly hasConflicts: boolean;
	/**
	 * Represents wether the the commit is considered integrated, local only,
	 * or local and remote with respect to the branch it belongs to.
	 * Note that remote only commits in the context of a branch are expressed with the [`UpstreamCommit`] struct instead of this.
	 */
	readonly state: CommitState;
	/** Commit creation time in Epoch milliseconds. */
	readonly createdAt: string;
	/** The author of the commit. */
	readonly author: Author;
};

/**
 * Commit that is only at the remote.
 * Unlike the `Commit` struct, there is no knowledge of GitButler concepts like conflicted state etc.
 */
export type UpstreamCommit = {
	/** The OID of the commit. */
	readonly id: string;
	/** The message of the commit. */
	readonly message: string;
	/** Commit creation time in Epoch milliseconds. */
	readonly createdAt: number;
	/** The author of the commit. */
	readonly author: Author;
};

/** Represents the author of a commit. */
export type Author = {
	/** The name from the git commit signature */
	readonly name: string;
	/** The email from the git commit signature */
	readonly email: string;
	/** A URL to a gravatar image for the email from the commit signature */
	readonly gravatarUrl: string;
};

/** Represents the state a commit could be in. */
export type CommitState =
	/** The commit is only local */
	| { readonly type: 'LocalOnly' }
	/**
	 * The commit is also present at the remote tracking branch.
	 * This is the commit state if:
	 *  - The commit has been pushed to the remote
	 *  - The commit has been copied from a remote commit (when applying a remote branch)
	 *
	 * This variant carries the remote commit id in the `subject` field.
	 * The remote commit id may be the same as the `id` or it may be different if the local commit has been rebased or updated in another way.
	 */
	| { readonly type: 'LocalAndRemote'; readonly subject: string }
	/**
	 * The commit is considered integrated.
	 * This should happen when this commit or the contents of this commit is already part of the base.
	 */
	| { readonly type: 'Integrated' };

export type CommitStateType = CommitState['type'];
