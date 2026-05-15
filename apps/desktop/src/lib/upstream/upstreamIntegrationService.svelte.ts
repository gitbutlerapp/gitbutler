import { InjectionToken } from "@gitbutler/core/context";
import { isDefined } from "@gitbutler/ui/utils/typeguards";
import type { StackService } from "$lib/stacks/stackService.svelte";
import type { BackendApi } from "$lib/state/backendApi";
import type { StackStatusesWithBranchesV3 } from "$lib/upstream/types";

export const UPSTREAM_INTEGRATION_SERVICE = new InjectionToken<UpstreamIntegrationService>(
	"UpstreamIntegrationService",
);

export class UpstreamIntegrationService {
	constructor(
		private backendApi: BackendApi,
		private stackService: StackService,
	) {}

	async upstreamStatuses(
		projectId: string,
		targetCommitOid: string | undefined,
	): Promise<StackStatusesWithBranchesV3 | undefined> {
		const stacks = await this.stackService.fetchStacks(projectId);
		const branchStatuses = await this.backendApi.endpoints.upstreamIntegrationStatuses.fetch({
			projectId,
			targetCommitOid,
		});

		if (branchStatuses.type === "upToDate") return branchStatuses;

		const stackStatusesWithBranches: StackStatusesWithBranchesV3 = {
			type: "updatesRequired",
			worktreeConflicts: branchStatuses.subject.worktreeConflicts,
			subject: branchStatuses.subject.statuses
				.map((status) => {
					const stack = stacks.find((appliedBranch) => appliedBranch.id === status[0]);

					if (!stack) return;
					return {
						stack,
						status: status[1],
					};
				})
				.filter(isDefined),
		};

		return stackStatusesWithBranches;
	}

	resolveUpstreamIntegration() {
		return this.backendApi.endpoints.resolveUpstreamIntegration.useMutation();
	}

	get resolveUpstreamIntegrationMutation() {
		return this.backendApi.endpoints.resolveUpstreamIntegration.mutate;
	}

	integrateUpstream() {
		return this.backendApi.endpoints.integrateUpstream.useMutation();
	}
}
