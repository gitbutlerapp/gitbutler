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
import type { Reactive } from '@gitbutler/shared/storeUtils';

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

	upstreamStatuses(
		projectId: string,
		targetCommitOid?: string
	): Reactive<StackStatusesWithBranchesV3 | undefined> {
		const stacks = this.stackService.stacks(projectId);
		const branchStatuses = this.api.endpoints.upstreamIntegrationStatuses.useQuery({
			projectId,
			targetCommitOid
		});

		const result = $derived.by(() => {
			if (!stacks.current.isSuccess || !branchStatuses.current.isSuccess) return;
			const stackData = stacks.current.data;
			const branchStatusesData = branchStatuses.current.data;
			if (branchStatusesData.type === 'upToDate') return branchStatusesData;

			const stackStatusesWithBranches: StackStatusesWithBranchesV3 = {
				type: 'updatesRequired',
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
		});

		return {
			get current() {
				return result;
			}
		};
	}

	resolveUpstreamIntegration() {
		return this.api.endpoints.resolveUpstreamIntegration.useMutation();
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
					command: `upstream_integration_statuses`,
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
				query: ({ projectId, resolutions, baseBranchResolution }) => ({
					command: `integrate_upstream`,
					params: { projectId, resolutions, baseBranchResolution }
				}),
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
				query: ({ projectId, resolutionApproach }) => ({
					command: `resolve_upstream_integration`,
					params: { projectId, resolutionApproach }
				}),
				invalidatesTags: [invalidatesList(ReduxTag.UpstreamIntegrationStatus)]
			})
		})
	});
}
