const BRANCH_SEPARATOR = '/';

export function getBranchNameFromRef(ref: string): string | undefined {
	return ref.split(BRANCH_SEPARATOR).pop();
}
