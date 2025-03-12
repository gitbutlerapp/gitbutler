import { ProjectsService } from '$lib/project/projectsService';
import { ReduxTag } from '$lib/state/tags';
import { BranchService as CloudBranchService } from '@gitbutler/shared/branches/branchService';
import { BranchStatus as CloudBranchStatus } from '@gitbutler/shared/branches/types';
import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type { Stack } from '$lib/stacks/stack';
import type { StackService } from '$lib/stacks/stackService.svelte';
import type { ClientState } from '$lib/state/clientState.svelte';
import type {
	BaseBranchResolution,
	BaseBranchResolutionApproach,
	BranchStatusesResponse,
	IntegrationOutcome,
	Resolution,
	StackStatusesWithBranchesV3
} from './types';
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
				subject: branchStatusesData.subject
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

	async resolveUpstreamIntegration(
		projectId: string,
		type: BaseBranchResolutionApproach
	): Promise<string | undefined> {
		const response = await this.api.endpoints.resolveUpstreamIntegration.useMutation({
			projectId,
			resolutionApproach: { type }
		});

		const result = $derived.by(() => {
			if (!response.data) {
				console.error(response.error);
				return;
			}

			return response.data;
		});

		return result;
	}

	async integrateUpstream(
		projectId: string,
		resolutions: Resolution[],
		stacks: Stack[],
		baseBranchResolution?: BaseBranchResolution
	): Promise<IntegrationOutcome | undefined> {
		const outcomeResponse = await this.api.endpoints.integrateUpstream.useMutation({
			projectId,
			resolutions,
			baseBranchResolution
		});

		const result = $derived.by(() => {
			// if (!stacks.current.isSuccess) return;
			if (!outcomeResponse.data) {
				console.error(outcomeResponse.error);
				return;
			}

			const outcome = outcomeResponse.data;
			// We don't want to await this
			this.closeArchivedButRequests(projectId, outcome.archivedBranches, stacks);

			return outcome;
		});

		return result;
	}

	private async closeArchivedButRequests(
		projectId: string,
		archivedBranches: string[],
		stacks: Stack[]
	) {
		const project = await this.projectsService.getProject(projectId);
		if (!project.api) return;
		const cloudProject = await this.cloudProjectService.getProject(project.api.repository_id);
		if (!cloudProject) return;

		for (const archivedBranchName of archivedBranches) {
			const reviewId = this.findReviewIdForStack(stacks, archivedBranchName);
			if (!reviewId) continue;
			const cloudBranch = await this.latestBranchLookupService.getBranch(
				cloudProject.owner,
				cloudProject.slug,
				reviewId
			);
			if (!cloudBranch) continue;
			this.cloudBranchService.updateBranch(cloudBranch.uuid, { status: CloudBranchStatus.Closed });
		}
	}

	private findReviewIdForStack(_stacks: Stack[], _name: string): string | undefined {
		// TODO: Get the review ID from the stack by its name
		return undefined;
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
				})
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
				invalidatesTags: [ReduxTag.Stacks, ReduxTag.StackBranches]
			}),
			resolveUpstreamIntegration: build.mutation<
				string,
				{ projectId: string; resolutionApproach: { type: BaseBranchResolutionApproach } }
			>({
				query: ({ projectId, resolutionApproach }) => ({
					command: `resolve_upstream_integration`,
					params: { projectId, resolutionApproach }
				})
			})
		})
	});
}
