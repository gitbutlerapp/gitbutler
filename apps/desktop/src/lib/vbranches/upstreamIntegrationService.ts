import { invoke } from '$lib/backend/ipc';
import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import { derived, readable, type Readable } from 'svelte/store';
import type { Project } from '$lib/backend/projects';
import type { BranchStack } from '$lib/vbranches/types';

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

export type StackStatusInfo = { stack: BranchStack; status: BranchStatus };

export type StackStatusesWithStacks =
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
	if (statusInfo.status.type === 'fullyIntegrated') {
		return { type: 'delete' };
	}

	if (statusInfo.stack.allowRebasing) {
		return { type: 'rebase' };
	}

	return { type: 'merge' };
}

export function sortStatusInfo(a: StackStatusInfo, b: StackStatusInfo): number {
	if (
		(a.status.type !== 'fullyIntegrated' && b.status.type !== 'fullyIntegrated') ||
		(a.status.type === 'fullyIntegrated' && b.status.type === 'fullyIntegrated')
	) {
		return (a.stack?.name || 'Unknown').localeCompare(b.stack?.name || 'Unknown');
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

	upstreamStatuses(targetCommitOid?: string): Readable<StackStatusesWithStacks | undefined> {
		const branchStatuses = readable<BranchStatusesResponse | undefined>(undefined, (set) => {
			invoke<BranchStatusesResponse>('upstream_integration_statuses', {
				projectId: this.project.id,
				targetCommitOid
			}).then(set);
		});

		const branchStatusesWithBranches = derived(
			[branchStatuses, this.virtualBranchService.branches],
			([branchStatuses, stacks]): StackStatusesWithStacks | undefined => {
				if (!branchStatuses || !stacks) return;
				if (branchStatuses.type === 'upToDate') return branchStatuses;

				return {
					type: 'updatesRequired',
					subject: branchStatuses.subject
						.map((status) => {
							const stack = stacks.find((appliedBranch) => appliedBranch.id === status[0]);

							if (!stack) return;

							return {
								stack,
								status: status[1]
							};
						})
						.filter(isDefined)
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
