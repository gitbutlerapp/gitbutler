import { invoke } from '$lib/backend/ipc';
import { VirtualBranchService } from '$lib/branches/virtualBranchService';
import { BranchService as CloudBranchService } from '@gitbutler/shared/branches/branchService';
import { BranchStatus as CloudBranchStatus } from '@gitbutler/shared/branches/types';
import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import { get } from 'svelte/store';
import type { BranchStack, PatchSeries } from '$lib/branches/branch';
import type { Project } from '$lib/project/project';
import type { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';

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

export type IntegrationOutcome = {
	archivedBranches: string[];
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
		private virtualBranchService: VirtualBranchService,
		private cloudBranchService: CloudBranchService,
		private cloudProjectService: CloudProjectService,
		private latestBranchLookupService: LatestBranchLookupService
	) {}

	async upstreamStatuses(targetCommitOid?: string): Promise<StackStatusesWithBranches | undefined> {
		const branchStatuses = await invoke<BranchStatusesResponse>('upstream_integration_statuses', {
			projectId: this.project.id,
			targetCommitOid
		});

		const branches = get(this.virtualBranchService.branches);

		if (!branchStatuses || !branches) return;
		if (branchStatuses.type === 'upToDate') return branchStatuses;

		const stackStatusesWithBranches: StackStatusesWithBranches = {
			type: 'updatesRequired',
			subject: branchStatuses.subject
				.map((status) => {
					const stack = branches.find((appliedBranch) => appliedBranch.id === status[0]);

					if (!stack) return;

					return {
						stack,
						status: status[1]
					};
				})
				.filter(isDefined)
		};

		return stackStatusesWithBranches;
	}

	async integrateUpstream(
		resolutions: Resolution[],
		baseBranchResolution?: BaseBranchResolution
	): Promise<IntegrationOutcome> {
		const outcome = await invoke<IntegrationOutcome>('integrate_upstream', {
			projectId: this.project.id,
			resolutions,
			baseBranchResolution
		});

		// We don't want to await this
		this.closeArchivedButRequests(outcome.archivedBranches);

		return outcome;
	}

	async resolveUpstreamIntegration(type: BaseBranchResolutionApproach) {
		return await invoke<string>('resolve_upstream_integration', {
			projectId: this.project.id,
			resolutionApproach: { type }
		});
	}

	private async closeArchivedButRequests(archivedBranches: string[]) {
		if (!this.project.api) return;
		const project = await this.cloudProjectService.getProject(this.project.api.repository_id);
		if (!project) return;

		const stacks = get(this.virtualBranchService.branches);
		if (!stacks) return;

		for (const archivedBranchName of archivedBranches) {
			const branch = this.findBranchWithName(stacks, archivedBranchName);
			if (!branch || !branch.reviewId) continue;
			const cloudBranch = await this.latestBranchLookupService.getBranch(
				project.owner,
				project.slug,
				branch.reviewId
			);
			if (!cloudBranch) continue;
			this.cloudBranchService.updateBranch(cloudBranch.uuid, { status: CloudBranchStatus.Closed });
		}
	}

	private findBranchWithName(stacks: BranchStack[], name: string): PatchSeries | undefined {
		for (const stack of stacks) {
			for (const branch of stack.series) {
				if (branch instanceof Error) continue;
				if (branch.name === name) return branch;
			}
		}
	}
}
