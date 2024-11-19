import { invoke } from '$lib/backend/ipc';
import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
import { derived, readable, type Readable } from 'svelte/store';
import type { Project } from '$lib/backend/projects';
import type { VirtualBranch } from '$lib/vbranches/types';

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

export type StackStatusInfo = { stack: VirtualBranch; status: StackStatus };

export type StackStatusesWithBranches =
	| {
			type: 'upToDate';
	  }
	| {
			type: 'updatesRequired';
			subject: StackStatusInfo[];
	  };

export type ResolutionApproach = {
	type: 'rebase' | 'merge' | 'unapply' | 'delete';
};

export type Resolution = {
	branchId: string;
	branchTree: string;
	approach: ResolutionApproach;
};

export type BaseBranchResolutionApproach = 'rebase' | 'merge' | 'hardReset';

export type BaseBranchResolution = {
	targetCommitOid: string;
	approach: { type: BaseBranchResolutionApproach };
};

export function getBaseBrancheResolution(
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

type BranchStatusesResponse =
	| {
			type: 'upToDate';
	  }
	| {
			type: 'updatesRequired';
			subject: [string, StackStatus][];
	  };

export class UpstreamIntegrationService {
	constructor(
		private project: Project,
		private virtualBranchService: VirtualBranchService
	) {}

	upstreamStatuses(targetCommitOid?: string): Readable<StackStatusesWithBranches | undefined> {
		const branchStatuses = readable<BranchStatusesResponse | undefined>(undefined, (set) => {
			invoke<BranchStatusesResponse>('upstream_integration_statuses', {
				projectId: this.project.id,
				targetCommitOid
			}).then(set);
		});

		const branchStatusesWithBranches = derived(
			[branchStatuses, this.virtualBranchService.branches],
			([branchStatuses, branches]): StackStatusesWithBranches | undefined => {
				if (!branchStatuses || !branches) return;
				if (branchStatuses.type === 'upToDate') return branchStatuses;

				return {
					type: 'updatesRequired',
					subject: branchStatuses.subject.map((status) => {
						const stack = branches.find((appliedBranch) => appliedBranch.id === status[0]);

						if (!stack) {
							throw new Error(
								`Could not find stack with id ${status[0]}. Please report this issue and try restarting the app and trying again.`
							);
						}

						return {
							stack,
							status: status[1]
						};
					})
				};
			}
		);

		return branchStatusesWithBranches;
	}

	async integrateUpstream(resolutions: Resolution[], baseBranchResolution?: BaseBranchResolution) {
		return await invoke('integrate_upstream', {
			projectId: this.project.id,
			resolutions,
			baseBranchResolution
		});
	}

	async resolveUpstreamIntegration(type: BaseBranchResolutionApproach) {
		return await invoke<string>('resolve_upstream_integration', {
			projectId: this.project.id,
			resolutionApproach: { type }
		});
	}
}
