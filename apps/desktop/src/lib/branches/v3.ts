import { invoke } from '$lib/backend/ipc';

/**
 * Returns the branches that belong to a particular `Stack`
 * The entries are ordered from newest to oldest.
 */
export async function stackBranches(
	projectId: string,
	stackId: string
): Promise<WorkspaceBranch[]> {
	return await invoke<WorkspaceBranch[]>('stack_branches', { projectId, stackId });
}

/**
 * Represents a branch in a `Stack`.
 * It contains commits derived from the local pseudo branch and it's respective remote.
 * This is distinct from a git branch in the sense that it has no local reference - something that we may want to change in the future.
 */
export type WorkspaceBranch = {
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
	 * A stack branch can be either in the stack or archived, which is what this field represents.
	 * Only branches that are currently in the stacked state will provide lists of commits.
	 */
	readonly state: State;
};

/** Represents the state of a branch in a stack. */
export type State =
	/**
	 * Archived indicates that the branch was previously part of a stack but it has since been integrated.
	 * In other words, the merge base of the stack is now above this branch.
	 * This would occur when the branch has been merged at the remote and the workspace has been updated with that change.
	 */
	| { readonly type: 'Archived' }
	/** Indicates that the branch is considered to be part of a stack and has commits associated with it. */
	| { readonly type: 'Stacked'; readonly subject: Commits };

/** List of commits beloning to this branch. Ordered from newest to oldest (child-most to parent-most). */
export type Commits = {
	/**
	 * Commits that are currently part of the workspace (applied).
	 * Created from the local pseudo branch (head currently stored in the TOML file)
	 *
	 * When there is only one branch in the stack, this includes the commits
	 * from the tip of the stack to the merge base with the trunk / target branch (not including the merge base).
	 *
	 * When there are multiple branches in the stack, this includes the commits from the branch head to the next branch in the stack.
	 *
	 * In either case this is effectively a list of commits that in the working copy which may or may not have been pushed to the remote.
	 */
	readonly localAndRemote: Commit[];

	/**
	 * List of commits that exist **only** on the upstream branch. Ordered from newest to oldest.
	 * Created from the tip of the local tracking branch eg. refs/remotes/origin/my-branch -> refs/heads/my-branch
	 *
	 * This does **not** include the commits that are in the commits list (local)
	 * This is effectively the list of commits that are on the remote branch but are not in the working copy.
	 */
	readonly upstreamOnly: UpstreamCommit[];
};

/** Commit that is a part of a [`StackBranch`](gitbutler_stack::StackBranch) and, as such, containing state derived in relation to the specific branch.*/
export type Commit = {
	/** The OID of the commit.*/
	readonly id: string;
	/** The message of the commit.*/
	readonly message: string;
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
