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

export type StackInfo = {
	id: string;
	name: string;
	/**
	 * Whether the stack contains any conflicted changes.
	 */
	isConflicted: boolean;
	/**
	 * Whether there are local changes that can be pushed to the remote.
	 */
	isDirty: boolean;
	/**
	 * Whether the stack requires a force push to the remote.
	 */
	requiresForce: boolean;
};
