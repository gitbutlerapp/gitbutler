import type { BaseBranch } from '$lib/vbranches/types';

export type CommitType = 'local' | 'remote' | 'integrated';

export function branchUrl(target: BaseBranch | undefined | null, upstreamBranchName: string) {
	if (!target) return undefined;
	const baseBranchName = target.branchName.split('/')[1];
	const parts = upstreamBranchName.split('/');
	const branchName = parts[parts.length - 1];
	return `${target.repoBaseUrl}/compare/${baseBranchName}...${branchName}`;
}
