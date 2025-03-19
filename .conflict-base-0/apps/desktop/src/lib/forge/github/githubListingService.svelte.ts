import { ghQuery } from './ghQuery';
import { ghResponseToInstance } from './types';
import { ReduxTag } from '$lib/state/tags';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { GitHubApi } from '$lib/state/clientState.svelte';
import type { ForgeListingService } from '../interface/forgeListingService';
import type { PullRequest } from '../interface/types';

export class GitHubListingService implements ForgeListingService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		gitHubApi: GitHubApi,
		private projectMetrics?: ProjectMetrics
	) {
		this.api = injectEndpoints(gitHubApi);
	}

	list(projectId: string) {
		const result = $derived(
			this.api.endpoints.listPrs.useQuery(undefined, {
				transform: (result) => prSelectors.selectAll(result)
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

	getByBranch(branchName: string) {
		const result = $derived(
			this.api.endpoints.listPrs.useQuery(undefined, {
				transform: (result) => prSelectors.selectById(result, branchName)
			})
		);
		return result;
	}

	async refresh() {
		await this.api.endpoints.listPrs.fetch();
	}
}

function injectEndpoints(api: GitHubApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listPrs: build.query<EntityState<PullRequest, string>, void>({
				queryFn: async (_, api) => {
					const result = await ghQuery({
						domain: 'pulls',
						action: 'list',
						extra: api.extra
					});
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
				providesTags: [ReduxTag.PullRequests]
			})
		})
	});
}

const prAdapter = createEntityAdapter<PullRequest, string>({
	selectId: (pr) => pr.sourceBranch
});

const prSelectors = prAdapter.getSelectors();

// if (err.message.includes('you appear to have the correct authorization credentials')) {
// 	this.disabled = true;
// }
