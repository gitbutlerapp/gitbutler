export function normalizeBranchName(value: string) {
	return value.replace(/[^0-9a-z/_.]+/g, '-');
}
