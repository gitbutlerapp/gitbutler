import { getStackName, type Stack } from '$lib/stacks/stack';
import type { BranchStack } from '$lib/branches/branch';

export type StackStatus = {
	treeStatus: TreeStatus;
	branchStatuses: NameAndBranchStatus[];
};

export type NameAndBranchStatus = {
	name: string;
	status: BranchStatus;
};

export type BranchStatus =
	| {
			type: 'empty' | 'integrated' | 'saflyUpdatable';
	  }
	| {
			type: 'conflicted';
			subject: {
				rebasable: boolean;
			};
	  };

export function stackFullyIntegrated(stackStatus: StackStatus): boolean {
	return (
		stackStatus.branchStatuses.every((branchStatus) => branchStatus.status.type === 'integrated') &&
		stackStatus.treeStatus.type === 'empty'
	);
}

export type TreeStatus = {
	type: 'empty' | 'conflicted' | 'saflyUpdatable';
};

export type StackStatusInfo = { stack: BranchStack; status: StackStatus };

export type StackStatusesWithBranches =
	| {
			type: 'upToDate';
	  }
	| {
			type: 'updatesRequired';
			subject: StackStatusInfo[];
	  };

export type StackStatusInfoV3 = { stack: Stack; status: StackStatus };

export type StackStatusesWithBranchesV3 =
	| {
			type: 'upToDate';
	  }
	| {
			type: 'updatesRequired';
			subject: StackStatusInfoV3[];
	  };

export type ResolutionApproach = {
	type: 'rebase' | 'merge' | 'unapply' | 'delete';
};

export type Resolution = {
	branchId: string;
	approach: ResolutionApproach;
	deleteIntegratedBranches: boolean;
};

export type BaseBranchResolutionApproach = 'rebase' | 'merge' | 'hardReset';

export type BaseBranchResolution = {
	targetCommitOid: string;
	approach: { type: BaseBranchResolutionApproach };
};

export type IntegrationOutcome = {
	archivedBranches: string[];
	reviewIdsToClose: string[];
};

export function getBaseBranchResolution(
	targetCommitOid: string | undefined,
	approach: BaseBranchResolutionApproach
): BaseBranchResolution | undefined {
	if (!targetCommitOid) return;

	return {
		targetCommitOid,
		approach: { type: approach }
	};
}

export function getResolutionApproach(statusInfo: StackStatusInfo): ResolutionApproach {
	if (stackFullyIntegrated(statusInfo.status)) {
		return { type: 'delete' };
	}

	if (statusInfo.stack.allowRebasing) {
		return { type: 'rebase' };
	}

	return { type: 'merge' };
}

export function sortStatusInfo(a: StackStatusInfo, b: StackStatusInfo): number {
	if (
		(!stackFullyIntegrated(a.status) && !stackFullyIntegrated(b.status)) ||
		(stackFullyIntegrated(a.status) && stackFullyIntegrated(b.status))
	) {
		return (a.stack?.name || 'Unknown').localeCompare(b.stack?.name || 'Unknown');
	}

	if (stackFullyIntegrated(a.status)) {
		return 1;
	} else {
		return -1;
	}
}

export function sortStatusInfoV3(a: StackStatusInfoV3, b: StackStatusInfoV3): number {
	if (
		(!stackFullyIntegrated(a.status) && !stackFullyIntegrated(b.status)) ||
		(stackFullyIntegrated(a.status) && stackFullyIntegrated(b.status))
	) {
		const aName = (a.stack && getStackName(a.stack)) ?? 'Unknown';
		const bName = (b.stack && getStackName(b.stack)) ?? 'Unknown';

		return aName.localeCompare(bName);
	}

	if (stackFullyIntegrated(a.status)) {
		return 1;
	} else {
		return -1;
	}
}

export function getResolutionApproachV3(statusInfo: StackStatusInfoV3): ResolutionApproach {
	if (stackFullyIntegrated(statusInfo.status)) {
		return { type: 'delete' };
	}

	// TODO: Do we need this?
	// if (statusInfo.stack.allowRebasing) {
	// 	return { type: 'rebase' };
	// }

	// return { type: 'merge' };
	return { type: 'rebase' };
}

export type BranchStatusesResponse =
	| {
			type: 'upToDate';
	  }
	| {
			type: 'updatesRequired';
			subject: { worktreeConflicts: string[]; statuses: [string, StackStatus][] };
	  };
