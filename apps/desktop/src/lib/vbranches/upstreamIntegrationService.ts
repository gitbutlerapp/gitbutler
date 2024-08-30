import { invoke } from '$lib/backend/ipc';
import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
import { derived, readable, type Readable } from 'svelte/store';
import type { Project } from '$lib/backend/projects';
import type { VirtualBranch } from '$lib/vbranches/types';

type BranchStatus =
	| {
			type: 'Empty' | 'FullyIntegrated' | 'SaflyUpdatable';
	  }
	| {
			type: 'Conflicted';
			subject: {
				potentiallyConflictedUncommitedChanges: boolean;
			};
	  };

type BranchStatuses =
	| {
			type: 'UpToDate';
	  }
	| {
			type: 'UpdatesRequired';
			subject: [string, BranchStatus][];
	  };

export type BranchStatusesWithBranches =
	| {
			type: 'UpToDate';
	  }
	| {
			type: 'UpdatesRequired';
			subject: { branch: VirtualBranch | undefined; status: BranchStatus }[];
	  };

export class UpstreamIntegrationService {
	constructor(
		private project: Project,
		private virtual_branch_service: VirtualBranchService
	) {}

	upstreamStatuses(): Readable<BranchStatusesWithBranches | undefined> {
		const branchStatuses = readable<BranchStatuses | undefined>(undefined, (set) => {
			invoke<BranchStatuses>('upstream_integration_statuses', { projectId: this.project.id }).then(
				set
			);
		});

		const branchStatusesWithBranches = derived(
			[branchStatuses, this.virtual_branch_service.branches],
			([branchStatuses, branches]): BranchStatusesWithBranches | undefined => {
				if (!branchStatuses || !branches) return;
				if (branchStatuses.type === 'UpToDate') return branchStatuses;

				return {
					type: 'UpdatesRequired',
					subject: branchStatuses.subject.map((status) => {
						const branch = branches.find((appliedBranch) => appliedBranch.id === status[0]);

						return {
							branch,
							status: status[1]
						};
					})
				};
			}
		);

		return branchStatusesWithBranches;
	}
}
