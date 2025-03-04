import { BranchStatus, type Branch, type Patch } from '@gitbutler/shared/branches/types';
import { type CommitStatusType } from '@gitbutler/ui/CommitStatusBadge.svelte';

function getPatches(branch: Branch): Patch[] {
	return branch.patches;
}

function anyRejected(patches: Patch[]): boolean {
	return patches.some((patch) => patch.reviewAll.rejected.length > 0);
}

function someApproved(patches: Patch[]): boolean {
	return patches.some((patch) => patch.reviewAll.signedOff.length > 0);
}

function allApproved(patches: Patch[]): boolean {
	return !patches.some((patch) => patch.reviewAll.signedOff.length === 0);
}

function hasComments(patches: Patch[]): boolean {
	return patches.some((patch) => patch.commentCount > 0);
}

export function getBranchStatusBadge(branch: Branch): CommitStatusType {
	const patches = getPatches(branch);

	if (branch.status === BranchStatus.Closed) {
		return 'closed';
	} else if (branch.status === BranchStatus.Loading) {
		return 'loading';
	} else if (anyRejected(patches)) {
		return 'changes-requested';
	} else if (allApproved(patches)) {
		return 'approved';
	} else if (someApproved(patches)) {
		return 'in-discussion';
	} else if (hasComments(patches) && !someApproved(patches) && !anyRejected(patches)) {
		return 'in-discussion';
	} else {
		return 'unreviewed';
	}
}
