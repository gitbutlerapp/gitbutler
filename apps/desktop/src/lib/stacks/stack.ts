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
	 * Cannot push. This is the case when the stack contains at least one conflicted commit.
	 */
	| 'conflictedCommits';

export type StackInfo = {
	/**
	 * This is the name of the top-most branch, provided by the API for convinience
	 */
	derivedName: string;
	/**
	 * The pushable status for the stack
	 */
	pushStatus: PushStatus;
};

export function stackRequiresForcePush(stack: StackInfo): boolean {
	return stack.pushStatus === 'unpushedCommitsRequiringForce';
}

export function stackHasConflicts(stack: StackInfo): boolean {
	return stack.pushStatus === 'conflictedCommits';
}

export function stackHasUnpushedCommits(stack: StackInfo): boolean {
	return (
		stack.pushStatus === 'unpushedCommits' || stack.pushStatus === 'unpushedCommitsRequiringForce'
	);
}
