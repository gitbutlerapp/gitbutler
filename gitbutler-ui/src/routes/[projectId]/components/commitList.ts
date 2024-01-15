import type { BaseBranch } from '$lib/vbranches/types';

export function branchUrl(
	target: BaseBranch | undefined | null,
	upstreamBranchName: string | undefined
) {
	if (!target || !upstreamBranchName) return undefined;
	const baseBranchName = target.branchName.split('/')[1];
	const parts = upstreamBranchName.split('/');
	const branchName = parts[parts.length - 1];
	return `${target.repoBaseUrl.trim()}/compare/${baseBranchName}...${branchName}`;
}
