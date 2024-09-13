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

type BranchStatuses =
	| {
			type: 'upToDate';
	  }
	| {
			type: 'updatesRequired';
			subject: [string, BranchStatus][];
	  };

export type BranchStatusesWithBranches =
	| {
			type: 'upToDate';
	  }
	| {
			type: 'updatesRequired';
			subject: { branch: VirtualBranch; status: BranchStatus }[];
	  };

export type ResolutionApproach = {
	type: 'rebase' | 'merge' | 'unapply' | 'delete';
};

export type Resolution = {
	branchId: string;
	branchTree: string;
	approach: ResolutionApproach;
};

export class UpstreamIntegrationService {
	constructor(
		private project: Project,
		private virtualBranchService: VirtualBranchService
	) {}

	upstreamStatuses(): Readable<BranchStatusesWithBranches | undefined> {
		const branchStatuses = readable<BranchStatuses | undefined>(undefined, (set) => {
			invoke<BranchStatuses>('upstream_integration_statuses', { projectId: this.project.id }).then(
				set
			);
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
		console.log(resolutions);
		return await invoke('integrate_upstream', { projectId: this.project.id, resolutions });
	}
}
