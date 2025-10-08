import type { TreeChange, TreeStats } from '$lib/hunks/change';

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
	readonly createdAt: number;
	/** The author of the commit. */
	readonly author: Author;
};

/** List of changes, stats and metadata for a commit */
export type CommitDetails = {
	/** The commit */
	readonly commit: Commit;
	/** The changes that were made to the tree. */
	readonly changes: TreeChange[];
	/** The stats of the changes. */
	readonly stats: TreeStats;
	/** If there are any conflicted files this will show them */
	readonly conflictEntries?: {
		ancestorEntries: string[];
		ourEntries: string[];
		theirEntries: string[];
	};
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

export function isCommit(something: Commit | UpstreamCommit): something is Commit {
	return 'state' in something;
}

export function extractUpstreamCommitId(commit: Commit | UpstreamCommit): string | undefined {
	if (isCommit(commit)) {
		if (commit.state.type === 'LocalAndRemote') {
			return commit.state.subject;
		}
	}
	return undefined;
}

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
