import type { CombinedBranch } from "$lib/branches/types";

export function normalizeBranchName(value: string) {
	return value.replace(/[^A-Za-z0-9_/.#]+/g, '-');
}

export function getBranchLink(b: CombinedBranch, projectId: string): string | undefined {
	if (b.vbranch?.active) return `/${projectId}/board/`;
	if (b.vbranch) return `/${projectId}/stashed/${b.vbranch.id}`;
	if (b.remoteBranch) return `/${projectId}/remote/${b?.displayName}`;
	if (b.pr) return `/${projectId}/pull/${b.pr.number}`;
}