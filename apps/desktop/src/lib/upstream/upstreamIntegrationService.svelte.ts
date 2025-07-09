import { showError } from '$lib/notifications/toasts';
import { ProjectsService } from '$lib/project/projectsService';
import { invalidatesList, providesList, ReduxTag } from '$lib/state/tags';
import { BranchService as CloudBranchService } from '@gitbutler/shared/branches/branchService';
import { BranchStatus as CloudBranchStatus } from '@gitbutler/shared/branches/types';
import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
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
import type { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';

export class UpstreamIntegrationService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		state: ClientState,
		private stackService: StackService,
		private projectsService: ProjectsService,
		private cloudProjectService: CloudProjectService,
		private cloudBranchService: CloudBranchService,
		private latestBranchLookupService: LatestBranchLookupService
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

	integrateUpstream(projectId: string) {
		return this.api.endpoints.integrateUpstream.useMutation({
			sideEffect: async (data) =>
				await this.closeArchivedButRequests(projectId, data.reviewIdsToClose)
		});
	}

	private async closeArchivedButRequests(projectId: string, reviewIdsToClose: string[]) {
		const project = await this.projectsService.getProject(projectId);
		if (!project.api) return;
		const cloudProject = await this.cloudProjectService.getProject(project.api.repository_id);
		if (!cloudProject) return;

		for (const reviewId of reviewIdsToClose) {
			const cloudBranch = await this.latestBranchLookupService.getBranch(
				cloudProject.owner,
				cloudProject.slug,
				reviewId
			);
			if (!cloudBranch) continue;
			this.cloudBranchService.updateBranch(cloudBranch.uuid, { status: CloudBranchStatus.Closed });
		}
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			upstreamIntegrationStatuses: build.query<
				BranchStatusesResponse | undefined,
				{ projectId: string; targetCommitOid?: string }
			>({
				query: ({ projectId, targetCommitOid }) => ({
					command: 'upstream_integration_statuses',
					params: { projectId, targetCommitOid }
				}),
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
				query: (args) => ({ params: args }),
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
				query: (args) => ({ params: args }),
				invalidatesTags: [invalidatesList(ReduxTag.UpstreamIntegrationStatus)]
			})
		})
	});
}
