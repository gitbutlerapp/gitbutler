export function normalizeBranchName(value: string) {
	return value.replace(/[^A-Za-z0-9_/.#]+/g, '-');
}
