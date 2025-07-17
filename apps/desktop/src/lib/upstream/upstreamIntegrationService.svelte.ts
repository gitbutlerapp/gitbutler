import { showError } from '$lib/notifications/toasts';
import { ProjectsService } from '$lib/project/projectsService';
import { invalidatesList, providesList, ReduxTag } from '$lib/state/tags';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type { StackService } from '$lib/stacks/stackService.svelte';
import type { ClientState } from '$lib/state/clientState.svelte';
import type {
	BaseBranchResolution,
	BaseBranchResolutionApproach,
	BranchStatusesResponse,
	IntegrationOutcome,
	Resolution,
	StackStatusesWithBranchesV3
} from '$lib/upstream/types';

export class UpstreamIntegrationService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		state: ClientState,
		private stackService: StackService,
		private projectsService: ProjectsService
	) {
		this.api = injectEndpoints(state.backendApi);
	}

	async upstreamStatuses(
		projectId: string,
		targetCommitOid: string | undefined
	): Promise<StackStatusesWithBranchesV3 | undefined> {
		const stacks = await this.stackService.fetchStacks(projectId);
		const branchStatuses = await this.api.endpoints.upstreamIntegrationStatuses.fetch({
			projectId,
			targetCommitOid
		});

		if (!stacks.data || !branchStatuses.data) {
			if (stacks.isError) {
				showError('Failed to fetch stacks', stacks.error);
			}
			if (branchStatuses.isError) {
				showError('Failed to fetch upstream integration statuses', branchStatuses.error);
			}
			return undefined;
		}

		const stackData = stacks.data;
		const branchStatusesData = branchStatuses.data;

		if (branchStatusesData.type === 'upToDate') return branchStatusesData;

		const stackStatusesWithBranches: StackStatusesWithBranchesV3 = {
			type: 'updatesRequired',
			worktreeConflicts: branchStatusesData.subject.worktreeConflicts,
			subject: branchStatusesData.subject.statuses
				.map((status) => {
					const stack = stackData.find((appliedBranch) => appliedBranch.id === status[0]);

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

	resolveUpstreamIntegration() {
		return this.api.endpoints.resolveUpstreamIntegration.useMutation();
	}

	get resolveUpstreamIntegrationMutation() {
		return this.api.endpoints.resolveUpstreamIntegration.mutate;
	}

	integrateUpstream() {
		return this.api.endpoints.integrateUpstream.useMutation();
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			upstreamIntegrationStatuses: build.query<
				BranchStatusesResponse | undefined,
				{ projectId: string; targetCommitOid?: string }
			>({
				extraOptions: { command: 'upstream_integration_statuses' },
				query: (args) => args,
				providesTags: [providesList(ReduxTag.UpstreamIntegrationStatus)]
			}),
			integrateUpstream: build.mutation<
				IntegrationOutcome,
				{
					projectId: string;
					resolutions: Resolution[];
					baseBranchResolution?: BaseBranchResolution;
				}
			>({
				extraOptions: {
					command: 'integrate_upstream',
					actionName: 'Integrate Upstream'
				},
				query: (args) => args,
				invalidatesTags: [
					invalidatesList(ReduxTag.UpstreamIntegrationStatus),
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.StackDetails)
				]
			}),
			resolveUpstreamIntegration: build.mutation<
				string,
				{ projectId: string; resolutionApproach: { type: BaseBranchResolutionApproach } }
			>({
				extraOptions: {
					command: `resolve_upstream_integration`,
					actionName: 'Resolve Integrate Upstream'
				},
				query: (args) => args,
				invalidatesTags: [invalidatesList(ReduxTag.UpstreamIntegrationStatus)]
			})
		})
	});
}
