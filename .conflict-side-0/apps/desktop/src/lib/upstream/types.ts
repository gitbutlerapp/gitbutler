import { getStackName, type Stack } from '$lib/stacks/stack';

export type StackStatus = {
	treeStatus: TreeStatus;
	branchStatuses: NameAndBranchStatus[];
};

type NameAndBranchStatus = {
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

type TreeStatus = {
	type: 'empty' | 'conflicted' | 'saflyUpdatable';
};

export type StackStatusInfoV3 = { stack: Stack; status: StackStatus };

export type StackStatusesWithBranchesV3 =
	| {
			type: 'upToDate';
	  }
	| {
			type: 'updatesRequired';
			worktreeConflicts: string[];
			subject: StackStatusInfoV3[];
	  };

export type ResolutionApproach = {
	type: 'rebase' | 'merge' | 'unapply' | 'delete';
};

export type Resolution = {
	stackId: string;
	approach: ResolutionApproach;
	deleteIntegratedBranches: boolean;
	forceIntegratedBranches: string[];
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
