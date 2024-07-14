export function normalizeBranchName(str: string) {
	let result = str.replace(/\s+/g, '-');

	result = result.replace(/^[-/]+|[-/]+$/g, '');

	return result;
}
