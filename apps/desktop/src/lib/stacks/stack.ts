import type { Author, Commit } from '$lib/branches/v3';

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
	/**
	 * The name of the branch
	 */
	name: string;
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
