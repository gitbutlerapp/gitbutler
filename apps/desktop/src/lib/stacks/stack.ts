import type { Author, Commit, UpstreamCommit } from '$lib/branches/v3';

/**
 * Return type of Tauri `stacks` command.
 */
export type Stack = {
	id: string;
	branchNames: string[];
};

export function getStackName(stack: Stack): string | undefined {
	const lastBranch = stack.branchNames[stack.branchNames.length - 1];
	return lastBranch;
}

/** Represents the pushable status for the current stack */
export type PushStatus =
	/**
	 * Can push, but there are no changes to be pushed
	 */
	| 'nothingToPush'
	/**
	 * Can push. This is the case when there are local changes that can be pushed to the remote.
	 */
	| 'unpushedCommits'
	/**
	 * Can push, but requires a force push to the remote because commits were rewritten.
	 */
	| 'unpushedCommitsRequiringForce'
	/**
	 * No commits have been pushed to the remote.
	 */
	| 'completelyUnpushed'
	/**
	 * Every commit is integrated into the base branch.
	 */
	| 'integrated';

export type BranchDetails = {
	/** The name of the branch */
	readonly name: string;
	/** Upstream reference, e.g. `refs/remotes/origin/base-branch-improvements` */
	readonly remoteTrackingBranch: string | null;
	/**
	 * Description of the branch.
	 * Can include arbitrary utf8 data, eg. markdown etc.
	 */
	readonly description: string | null;
	/** The pull(merge) request associated with the branch, or None if no such entity has not been created. */
	readonly prNumber: number | null;
	/** A unique identifier for the GitButler review associated with the branch, if any. */
	readonly reviewId: string | null;
	/**
	 * This is the base commit from the perspective of this branch.
	 * If the branch is part of a stack and is on top of another branch, this is the head of the branch below it.
	 * If this branch is at the bottom of the stack, this is the merge base of the stack.
	 */
	readonly baseCommit: string;
	/**
	 * The pushable status for the branch
	 */
	pushStatus: PushStatus;
	/**
	 * The last time the branch was updated in Epoch milliseconds
	 */
	lastUpdatedAt: number;
	/**
	 * The authors of the commits in the branch
	 */
	authors: Author[];
	/**
	 * Whether any of the commits contained has conflicts
	 */
	isConflicted: boolean;
	/**
	 *  The commits contained in the branch, excluding the upstream commits.
	 */
	commits: Commit[];
	/**
	 * The commits that are only upstream.
	 */
	upstreamCommits: UpstreamCommit[];
};

export type StackDetails = {
	/**
	 * This is the name of the top-most branch, provided by the API for convinience
	 */
	derivedName: string;
	/**
	 * The pushable status for the stack
	 */
	pushStatus: PushStatus;
	/**
	 * The branches that make up the stack
	 */
	branchDetails: BranchDetails[];
	/**
	 * Whether any of the commits contained has conflicts
	 */
	isConflicted: boolean;
};

export function stackRequiresForcePush(stack: StackDetails): boolean {
	return stack.pushStatus === 'unpushedCommitsRequiringForce';
}

export function stackHasConflicts(stack: StackDetails): boolean {
	return stack.isConflicted;
}

export function stackHasUnpushedCommits(stack: StackDetails): boolean {
	return (
		stack.pushStatus === 'unpushedCommits' ||
		stack.pushStatus === 'unpushedCommitsRequiringForce' ||
		stack.pushStatus === 'completelyUnpushed'
	);
}

export function stackIsIntegrated(stack: StackDetails): boolean {
	return stack.pushStatus === 'integrated';
}
