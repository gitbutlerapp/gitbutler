import { invoke } from '$lib/backend/ipc';
import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import { derived, readable, type Readable } from 'svelte/store';
import type { Project } from '$lib/backend/projects';
import type { VirtualBranch } from '$lib/vbranches/types';

export type BranchStatus =
	| {
			type: 'empty' | 'fullyIntegrated' | 'saflyUpdatable';
	  }
	| {
			type: 'conflicted';
			subject: {
				potentiallyConflictedUncommitedChanges: boolean;
			};
	  };

export type BranchStatusInfo = { branch: VirtualBranch; status: BranchStatus };

export type BranchStatusesWithBranches =
	| {
			type: 'upToDate';
	  }
	| {
			type: 'updatesRequired';
			subject: BranchStatusInfo[];
	  };

export type ResolutionApproach = {
	type: 'rebase' | 'merge' | 'unapply' | 'delete';
};

export type Resolution = {
	branchId: string;
	branchTree: string;
	approach: ResolutionApproach;
};

export function getResolutionApproach(statusInfo: BranchStatusInfo): ResolutionApproach {
	if (statusInfo.status.type === 'fullyIntegrated') {
		return { type: 'delete' };
	}

	if (statusInfo.branch.allowRebasing) {
		return { type: 'rebase' };
	}

	return { type: 'merge' };
}

export function sortStatusInfo(a: BranchStatusInfo, b: BranchStatusInfo): number {
	if (
		(a.status.type !== 'fullyIntegrated' && b.status.type !== 'fullyIntegrated') ||
		(a.status.type === 'fullyIntegrated' && b.status.type === 'fullyIntegrated')
	) {
		return (a.branch?.name || 'Unknown').localeCompare(b.branch?.name || 'Unknown');
	}

	if (a.status.type === 'fullyIntegrated') {
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
			subject: [string, BranchStatus][];
	  };

export class UpstreamIntegrationService {
	constructor(
		private project: Project,
		private virtualBranchService: VirtualBranchService
	) {}

	upstreamStatuses(): Readable<BranchStatusesWithBranches | undefined> {
		const branchStatuses = readable<BranchStatusesResponse | undefined>(undefined, (set) => {
			invoke<BranchStatusesResponse>('upstream_integration_statuses', {
				projectId: this.project.id
			}).then(set);
		});

		const branchStatusesWithBranches = derived(
			[branchStatuses, this.virtualBranchService.branches],
			([branchStatuses, branches]): BranchStatusesWithBranches | undefined => {
				if (!branchStatuses || !branches) return;
				if (branchStatuses.type === 'upToDate') return branchStatuses;

				return {
					type: 'updatesRequired',
					subject: branchStatuses.subject
						.map((status) => {
							const branch = branches.find((appliedBranch) => appliedBranch.id === status[0]);

							if (!branch) return;

							return {
								branch,
								status: status[1]
							};
						})
						.filter(isDefined)
				};
			}
		);

		return branchStatusesWithBranches;
	}

	async integrateUpstream(resolutions: Resolution[]) {
		return await invoke('integrate_upstream', { projectId: this.project.id, resolutions });
	}
}
