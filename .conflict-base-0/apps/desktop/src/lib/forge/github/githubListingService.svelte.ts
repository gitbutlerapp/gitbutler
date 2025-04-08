import { ghQuery } from '$lib/forge/github/ghQuery';
import { ghResponseToInstance } from '$lib/forge/github/types';
import { createSelectByIds } from '$lib/state/customSelectors';
import { providesList, ReduxTag } from '$lib/state/tags';
import { reactive } from '@gitbutler/shared/storeUtils';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
import type { ForgeListingService } from '$lib/forge/interface/forgeListingService';
import type { PullRequest } from '$lib/forge/interface/types';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { GitHubApi } from '$lib/state/clientState.svelte';

export class GitHubListingService implements ForgeListingService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		gitHubApi: GitHubApi,
		private projectMetrics?: ProjectMetrics
	) {
		this.api = injectEndpoints(gitHubApi);
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
		return result;
	}

	getByBranch(projectId: string, branchName: string) {
		const result = $derived(
			this.api.endpoints.listPrs.useQuery(projectId, {
				transform: (result) => prSelectors.selectById(result, branchName)
			})
		);
		return result;
	}

	filterByBranch(projectId: string, branchName: string[]) {
		const result = $derived(
			this.api.endpoints.listPrs.useQueryState(projectId, {
				transform: (result) => prSelectors.selectByIds(result, branchName)
			})
		);
		const data = $derived(result.current.data);
		return reactive(() => data || []);
	}

	async refresh(projectId: string): Promise<void> {
		await this.api.endpoints.listPrs.fetch(projectId, { forceRefetch: true });
	}
}

function injectEndpoints(api: GitHubApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listPrs: build.query<EntityState<PullRequest, string>, string>({
				queryFn: async (_, api) => {
					const result = await ghQuery<'pulls', 'list', 'required'>(
						async (octokit, repository) => ({
							data: await octokit.paginate(octokit.rest.pulls.list, repository)
						}),
						api.extra,
						'required'
					);

					if (result.data) {
						return {
							data: prAdapter.addMany(
								prAdapter.getInitialState(),
								result.data.map((item) => ghResponseToInstance(item))
							)
						};
					}
					return result;
				},
				providesTags: [providesList(ReduxTag.PullRequests)]
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
