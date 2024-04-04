export function normalizeBranchName(value: string) {
	return value.toLowerCase().replace(/[^0-9a-z/_.]+/g, '-');
}
