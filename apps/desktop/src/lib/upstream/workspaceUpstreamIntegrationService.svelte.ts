import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/backendApi";
import type { WorkspaceBottomUpdate } from "$lib/upstream/workspaceUpstreamIntegration";
import type { RefInfo, WorkspaceState } from "@gitbutler/but-sdk";

export const WORKSPACE_UPSTREAM_INTEGRATION_SERVICE =
	new InjectionToken<WorkspaceUpstreamIntegrationService>("WorkspaceUpstreamIntegrationService");

export class WorkspaceUpstreamIntegrationService {
	constructor(private backendApi: BackendApi) {}

	headInfo(projectId: string) {
		return this.backendApi.endpoints.workspaceHeadInfo.useQuery({ projectId });
	}

	async fetchHeadInfo(projectId: string): Promise<RefInfo> {
		return await this.backendApi.endpoints.workspaceHeadInfo.fetch(
			{ projectId },
			{ forceRefetch: true },
		);
	}

	async preview(args: {
		projectId: string;
		updates: WorkspaceBottomUpdate[];
	}): Promise<WorkspaceState> {
		return await this.backendApi.endpoints.workspaceIntegrateUpstreamPreview.fetch(args, {
			forceRefetch: true,
		});
	}

	integrateUpstream() {
		return this.backendApi.endpoints.workspaceIntegrateUpstream.useMutation();
	}

	get deleteLocalBranch() {
		return this.backendApi.endpoints.deleteLocalBranch.mutate;
	}
}
