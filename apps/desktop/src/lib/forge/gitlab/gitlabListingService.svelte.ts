import { gitlab } from '$lib/forge/gitlab/gitlabClient.svelte';
import { mrToInstance } from '$lib/forge/gitlab/types';
import { createSelectByIds } from '$lib/state/customSelectors';
import { providesList, ReduxTag } from '$lib/state/tags';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
import type { ForgeListingService } from '$lib/forge/interface/forgeListingService';
import type { PullRequest } from '$lib/forge/interface/types';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { GitLabApi } from '$lib/state/clientState.svelte';

export class GitLabListingService implements ForgeListingService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		gitLabApi: GitLabApi,
		private projectMetrics?: ProjectMetrics
	) {
		this.api = injectEndpoints(gitLabApi);
	}

	list(projectId: string, pollingInterval?: number) {
		const result = $derived(
			this.api.endpoints.listPrs.useQuery(projectId, {
				transform: (result) => prSelectors.selectAll(result),
				subscriptionOptions: { pollingInterval }
			})
		);
		$effect(() => {
			const items = result.current.data;
			if (items) {
				this.projectMetrics?.setMetric(projectId, 'pr_count', items.length);
			}
		});
		return reactive(() => result.current);
	}

	getByBranch(projectId: string, branchName: string) {
		return this.api.endpoints.listPrs.useQuery(projectId, {
			transform: (result) => prSelectors.selectById(result, branchName)
		});
	}

	filterByBranch(projectId: string, branchName: string[]) {
		return this.api.endpoints.listPrs.useQueryState(projectId, {
			transform: (result) => prSelectors.selectByIds(result, branchName)
		});
	}

	async fetchByBranch(projectId: string, branchNames: string[]) {
		const results = await Promise.all(
			branchNames.map((branch) =>
				this.api.endpoints.listPrsByBranch.fetch({ projectId, branchName: branch })
			)
		);
		return results.filter(isDefined) ?? [];
	}

	async refresh(projectId: string): Promise<void> {
		await this.api.endpoints.listPrs.fetch(projectId, { forceRefetch: true });
	}
}

function injectEndpoints(api: GitLabApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listPrs: build.query<EntityState<PullRequest, string>, string>({
				queryFn: async (_, query) => {
					const { api, upstreamProjectId, forkProjectId } = gitlab(query.extra);
					const upstreamMrs = await api.MergeRequests.all({
						projectId: upstreamProjectId,
						state: 'opened'
					});
					const forkMrs = await api.MergeRequests.all({
						projectId: forkProjectId,
						state: 'opened'
					});

					return {
						data: prAdapter.addMany(
							prAdapter.getInitialState(),
							[...upstreamMrs, ...forkMrs].map((mr) => mrToInstance(mr))
						)
					};
				},
				providesTags: [providesList(ReduxTag.PullRequests)]
			}),
			listPrsByBranch: build.query<PullRequest | null, { projectId: string; branchName: string }>({
				queryFn: async ({ branchName }, query) => {
					const { api, upstreamProjectId, forkProjectId } = gitlab(query.extra);
					const upstreamMrs = await api.MergeRequests.all({
						projectId: upstreamProjectId,
						sourceBranch: branchName,
						state: 'opened'
					});
					const forkMrs = await api.MergeRequests.all({
						projectId: forkProjectId,
						sourceBranch: branchName,
						state: 'opened'
					});

					const allMrs = [...upstreamMrs, ...forkMrs];
					if (allMrs.length === 0) {
						return { data: null };
					}

					if (allMrs.length > 1) {
						return { error: new Error(`Multiple merge requests found for branch ${branchName}`) };
					}

					const mrData = allMrs[0]!;
					const mr = mrToInstance(mrData);

					return {
						data: mr
					};
				}
			})
		})
	});
}

const prAdapter = createEntityAdapter<PullRequest, string>({
	selectId: (pr) => pr.sourceBranch
});

const prSelectors = { ...prAdapter.getSelectors(), selectByIds: createSelectByIds<PullRequest>() };

// if (err.message.includes('you appear to have the correct authorization credentials')) {
// 	this.disabled = true;
// }
