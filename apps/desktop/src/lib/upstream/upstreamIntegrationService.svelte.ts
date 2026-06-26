import {
	buildUpstreamIntegrationUpdates,
	deriveUpstreamIntegrationStatuses,
	type UpstreamIntegrationStatuses,
} from "$lib/upstream/types";
import { InjectionToken } from "@gitbutler/core/context";
import type { PrService } from "$lib/forge/prService.svelte";
import type { StackService } from "$lib/stacks/stackService.svelte";
import type { BackendApi } from "$lib/state/backendApi";

export const UPSTREAM_INTEGRATION_SERVICE = new InjectionToken<UpstreamIntegrationService>(
	"UpstreamIntegrationService",
);

export class UpstreamIntegrationService {
	constructor(
		private backendApi: BackendApi,
		private stackService: StackService,
		private prService: PrService,
	) {}

	async upstreamStatuses(projectId: string): Promise<UpstreamIntegrationStatuses> {
		await this.prService.waitForRefreshes(projectId);

		const stacks = await this.stackService.fetchStacks(projectId);
		const updates = buildUpstreamIntegrationUpdates(stacks);

		const preview = await this.backendApi.endpoints.workspaceIntegrateUpstream.mutate({
			projectId,
			updates,
			dryRun: true,
		});

		return {
			subject: deriveUpstreamIntegrationStatuses(stacks, preview.workspaceState.headInfo),
			updates,
			worktreeConflicts: preview.worktreeConflicts,
		};
	}

	resolveUpstreamIntegration() {
		return this.backendApi.endpoints.resolveUpstreamIntegration.useMutation();
	}

	get resolveUpstreamIntegrationMutation() {
		return this.backendApi.endpoints.resolveUpstreamIntegration.mutate;
	}

	integrateUpstream() {
		return this.backendApi.endpoints.workspaceIntegrateUpstream.useMutation();
	}
}
